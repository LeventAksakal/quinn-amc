use std::{
    collections::HashSet,
    net::SocketAddr,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use amc_core::{
    AmcControllerConfig, AmcControllerEvent, AmcControllerPhase, AmcControllerSnapshot,
    DefaultUtilityScorer, Importance, MediaSemantics, RuntimeUtilityState, TrafficClass,
    UTILITY_SIGNAL_EWMA_WEIGHT, UtilityInputs, UtilityScorer, UtilitySignal,
};
use anyhow::{Context, Result, anyhow};
use clap::{Parser, ValueEnum};
use quinn::{ClientConfig, Endpoint};
use serde::{Deserialize, Serialize};
use tokio::{
    fs,
    time::{Duration, sleep_until},
};
use tracing::info;

const REPLAY_MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize)]
struct ReplayManifest {
    #[serde(default)]
    schema_version: Option<u32>,
    asset_name: String,
    init_segment: String,
    #[serde(default)]
    semantic_defaults: ReplaySemanticDefaults,
    segments: Vec<ReplaySegment>,
}

#[derive(Clone, Debug, Deserialize)]
struct ReplaySegment {
    sequence: u64,
    relative_path: String,
    start_time_ms: u64,
    duration_ms: u64,
    size_bytes: u64,
    #[serde(default)]
    semantic_hint: Option<ReplaySegmentSemanticHint>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct ReplaySemanticDefaults {
    startup_segment_count: Option<u64>,
    default_dependency_depth_hint: Option<u8>,
    default_freshness_window_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
struct ReplaySegmentSemanticHint {
    importance_hint: Option<Importance>,
    dependency_depth_hint: Option<u8>,
    independent: Option<bool>,
    freshness_window_ms: Option<u64>,
}

#[derive(Clone, Debug)]
struct PreparedInitSegment {
    relative_path: String,
    payload: Vec<u8>,
}

#[derive(Clone, Debug)]
struct PreparedReplaySegment {
    sequence: u64,
    relative_path: String,
    start_time_ms: u64,
    duration_ms: u64,
    payload: Vec<u8>,
    semantic_hint: Option<ReplaySegmentSemanticHint>,
}

#[derive(Clone, Debug)]
pub struct PreparedReplayInput {
    asset_name: String,
    semantic_defaults: ReplaySemanticDefaults,
    init_segment: PreparedInitSegment,
    segments: Vec<PreparedReplaySegment>,
}

#[derive(Clone, Debug)]
pub struct ReplayAssetInventory {
    pub asset_name: String,
    pub schema_version: u32,
    pub init_segment_path: PathBuf,
    pub segment_paths: Vec<PathBuf>,
    pub total_payload_bytes: u64,
}

struct ReplayInspection {
    manifest: ReplayManifest,
    asset_root: PathBuf,
    inventory: ReplayAssetInventory,
}

#[derive(Clone, Copy, Debug)]
struct RuntimeUtilityProfile {
    importance: Importance,
    dependency_depth: u8,
    independent: bool,
    freshness_window_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentKind {
    Init,
    Media,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum ReplayMode {
    Vod,
    Live,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum Pace {
    Immediate,
    Realtime,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum BaselineController {
    #[value(alias = "amc_preview")]
    AmcPreview,
    Bbr,
    #[default]
    Cubic,
    #[value(alias = "new_reno")]
    NewReno,
}

impl BaselineController {
    fn factory(
        self,
        runtime_utility: Arc<RuntimeUtilityState>,
    ) -> Arc<dyn quinn::congestion::ControllerFactory + Send + Sync> {
        match self {
            Self::AmcPreview => {
                Arc::new(AmcControllerConfig::default().with_runtime_state(runtime_utility))
            }
            Self::Bbr => Arc::new(quinn::congestion::BbrConfig::default()),
            Self::Cubic => Arc::new(quinn::congestion::CubicConfig::default()),
            Self::NewReno => Arc::new(quinn::congestion::NewRenoConfig::default()),
        }
    }

    fn uses_runtime_utility(self) -> bool {
        matches!(self, Self::AmcPreview)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RuntimeUtilityTelemetry {
    pub traffic_class: TrafficClass,
    pub importance: Importance,
    pub dependency_depth: u8,
    pub dependency_ready: bool,
    pub queue_delay_ms: u64,
    pub estimated_rtt_ms: u64,
    pub utility_score: f64,
    #[serde(default)]
    pub observed_utility_score: Option<f64>,
    #[serde(default)]
    pub smoothed_utility_score: Option<f64>,
    #[serde(default)]
    pub ewma_weight: Option<f64>,
    pub ack_gain: f64,
    pub loss_reduction_factor: f64,
    #[serde(default)]
    pub controller_snapshot: Option<AmcControllerSnapshotTelemetry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AmcControllerSnapshotTelemetry {
    pub phase: String,
    pub last_event: String,
    pub current_mtu_bytes: u64,
    pub congestion_window_bytes: u64,
    pub congestion_window_datagrams: u64,
    #[serde(default)]
    pub ssthresh_bytes: Option<u64>,
    #[serde(default)]
    pub ssthresh_datagrams: Option<u64>,
    pub initial_window_bytes: u64,
    pub initial_window_datagrams: u64,
    pub min_window_bytes: u64,
    pub min_window_datagrams: u64,
    pub max_window_bytes: u64,
    pub max_window_datagrams: u64,
    pub class_max_window_bytes: u64,
    pub class_max_window_datagrams: u64,
    pub growth_step_bytes: u64,
    pub growth_step_datagrams: u64,
}

#[derive(Debug, Deserialize, Serialize)]
struct SegmentHeader {
    asset_name: String,
    baseline_controller: BaselineController,
    mode: ReplayMode,
    kind: SegmentKind,
    sequence: u64,
    start_time_ms: u64,
    duration_ms: u64,
    deadline_ms: u64,
    client_send_elapsed_ms: u64,
    payload_len: u64,
    segment_path: String,
    runtime_utility: Option<RuntimeUtilityTelemetry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TransferSummary {
    pub asset_name: String,
    pub baseline_controller: BaselineController,
    pub segments_received: usize,
    pub media_segments_received: usize,
    pub total_payload_bytes: u64,
    pub useful_media_segments: usize,
    pub late_media_segments: usize,
    pub max_observed_lateness_ms: i64,
    pub amc_runtime_samples: usize,
    pub max_runtime_utility_score: Option<f64>,
    pub min_runtime_utility_score: Option<f64>,
    pub report_path: String,
}

#[derive(Debug, Parser, Clone)]
pub struct Args {
    #[arg(long, default_value = "0.0.0.0:0")]
    pub bind: SocketAddr,

    #[arg(long, default_value = "127.0.0.1:5000")]
    pub server: SocketAddr,

    #[arg(long, default_value = "localhost")]
    pub server_name: String,

    #[arg(long, default_value = "demo-cert.der")]
    pub cert: PathBuf,

    #[arg(
        long,
        default_value = "data/processed/manifests/big_buck_bunny_replay.json"
    )]
    pub replay_manifest: PathBuf,

    #[arg(long, value_enum, default_value = "immediate")]
    pub pace: Pace,

    #[arg(long, value_enum, default_value = "vod")]
    pub mode: ReplayMode,

    #[arg(long, default_value_t = 30_000)]
    pub vod_deadline_slack_ms: u64,

    #[arg(long, value_enum, default_value = "cubic")]
    pub controller: BaselineController,
}

pub async fn run(args: Args) -> Result<TransferSummary> {
    let replay_input = prepare_replay_input(&args.replay_manifest).await?;
    let cert_der = fs::read(&args.cert)
        .await
        .with_context(|| format!("failed to read {}", args.cert.display()))?;
    run_prepared(args, &replay_input, &cert_der).await
}

pub async fn run_prepared(
    args: Args,
    replay_input: &PreparedReplayInput,
    cert_der: &[u8],
) -> Result<TransferSummary> {
    let runtime_utility = Arc::new(RuntimeUtilityState::default());
    let client_config =
        build_client_config_from_cert_der(cert_der, args.controller, runtime_utility.clone())?;

    let mut endpoint = Endpoint::client(args.bind)
        .with_context(|| format!("failed to bind client endpoint on {}", args.bind))?;
    endpoint.set_default_client_config(client_config);

    let connection = endpoint
        .connect(args.server, &args.server_name)
        .with_context(|| {
            format!(
                "failed to start connection to {} using server name {}",
                args.server, args.server_name
            )
        })?
        .await
        .context("client connection failed")?;

    info!(server = %args.server, asset = %replay_input.asset_name, pace = ?args.pace, mode = ?args.mode, "connected to server");

    let (mut send, mut recv) = connection
        .open_bi()
        .await
        .context("failed to open stream")?;

    send_segment(
        &connection,
        &mut send,
        &replay_input.asset_name,
        args.controller,
        args.mode,
        SegmentKind::Init,
        0,
        0,
        0,
        0,
        0,
        &replay_input.init_segment.payload,
        &replay_input.init_segment.relative_path,
        &replay_input.semantic_defaults,
        None,
        runtime_utility.as_ref(),
    )
    .await?;

    let replay_start = Instant::now();
    send_media_segments(
        &connection,
        &mut send,
        replay_input,
        &args,
        replay_start,
        runtime_utility.as_ref(),
    )
    .await?;

    send.finish().context("failed to finish request stream")?;

    let response = recv
        .read_to_end(64 * 1024)
        .await
        .context("failed to read response")?;
    let response: TransferSummary =
        serde_json::from_slice(&response).context("response was not valid summary JSON")?;

    info!(
        asset = %response.asset_name,
        segments_received = response.segments_received,
        media_segments_received = response.media_segments_received,
        total_payload_bytes = response.total_payload_bytes,
        useful_media_segments = response.useful_media_segments,
        late_media_segments = response.late_media_segments,
        max_observed_lateness_ms = response.max_observed_lateness_ms,
        amc_runtime_samples = response.amc_runtime_samples,
        max_runtime_utility_score = response.max_runtime_utility_score,
        min_runtime_utility_score = response.min_runtime_utility_score,
        report_path = %response.report_path,
        "received transfer summary"
    );
    connection.close(0u32.into(), b"transfer complete");
    endpoint.wait_idle().await;
    Ok(response)
}

async fn send_media_segments(
    connection: &quinn::Connection,
    send: &mut quinn::SendStream,
    replay_input: &PreparedReplayInput,
    args: &Args,
    replay_start: Instant,
    runtime_utility: &RuntimeUtilityState,
) -> Result<()> {
    if use_amc_live_scheduler(args) {
        send_media_segments_with_amc_scheduler(
            connection,
            send,
            replay_input,
            args,
            replay_start,
            runtime_utility,
        )
        .await
    } else {
        send_media_segments_in_manifest_order(
            connection,
            send,
            replay_input,
            args,
            replay_start,
            runtime_utility,
        )
        .await
    }
}

async fn send_media_segments_in_manifest_order(
    connection: &quinn::Connection,
    send: &mut quinn::SendStream,
    replay_input: &PreparedReplayInput,
    args: &Args,
    replay_start: Instant,
    runtime_utility: &RuntimeUtilityState,
) -> Result<()> {
    for segment in &replay_input.segments {
        if matches!(args.pace, Pace::Realtime) {
            let release = tokio::time::Instant::from_std(
                replay_start + Duration::from_millis(segment.start_time_ms),
            );
            sleep_until(release).await;
        }

        send_prepared_segment(
            connection,
            send,
            replay_input,
            args,
            replay_start,
            segment,
            runtime_utility,
        )
        .await?;
    }

    Ok(())
}

async fn send_media_segments_with_amc_scheduler(
    connection: &quinn::Connection,
    send: &mut quinn::SendStream,
    replay_input: &PreparedReplayInput,
    args: &Args,
    replay_start: Instant,
    runtime_utility: &RuntimeUtilityState,
) -> Result<()> {
    let mut next_release_index = 0usize;
    let mut ready_indices = Vec::new();

    while next_release_index < replay_input.segments.len() || !ready_indices.is_empty() {
        let client_send_elapsed_ms = replay_start.elapsed().as_millis() as u64;

        while next_release_index < replay_input.segments.len()
            && replay_input.segments[next_release_index].start_time_ms <= client_send_elapsed_ms
        {
            ready_indices.push(next_release_index);
            next_release_index += 1;
        }

        if ready_indices.is_empty() {
            let next_segment = &replay_input.segments[next_release_index];
            let release = tokio::time::Instant::from_std(
                replay_start + Duration::from_millis(next_segment.start_time_ms),
            );
            sleep_until(release).await;
            continue;
        }

        let ready_position = select_amc_ready_segment_position(
            connection,
            replay_input,
            args,
            &ready_indices,
            client_send_elapsed_ms,
        );
        let segment_index = ready_indices.remove(ready_position);
        let segment = &replay_input.segments[segment_index];

        send_prepared_segment(
            connection,
            send,
            replay_input,
            args,
            replay_start,
            segment,
            runtime_utility,
        )
        .await?;
    }

    Ok(())
}

async fn send_prepared_segment(
    connection: &quinn::Connection,
    send: &mut quinn::SendStream,
    replay_input: &PreparedReplayInput,
    args: &Args,
    replay_start: Instant,
    segment: &PreparedReplaySegment,
    runtime_utility: &RuntimeUtilityState,
) -> Result<()> {
    send_segment(
        connection,
        send,
        &replay_input.asset_name,
        args.controller,
        args.mode,
        SegmentKind::Media,
        segment.sequence,
        segment.start_time_ms,
        segment.duration_ms,
        compute_deadline_ms(
            args.mode,
            segment.start_time_ms,
            segment.duration_ms,
            args.vod_deadline_slack_ms,
        ),
        replay_start.elapsed().as_millis() as u64,
        &segment.payload,
        &segment.relative_path,
        &replay_input.semantic_defaults,
        segment.semantic_hint.as_ref(),
        runtime_utility,
    )
    .await
}

fn use_amc_live_scheduler(args: &Args) -> bool {
    matches!(args.controller, BaselineController::AmcPreview)
        && matches!(args.mode, ReplayMode::Live)
        && matches!(args.pace, Pace::Realtime)
}

fn select_amc_ready_segment_position(
    connection: &quinn::Connection,
    replay_input: &PreparedReplayInput,
    args: &Args,
    ready_indices: &[usize],
    client_send_elapsed_ms: u64,
) -> usize {
    ready_indices
        .iter()
        .enumerate()
        .max_by(|(_, left_index), (_, right_index)| {
            let left_segment = &replay_input.segments[**left_index];
            let right_segment = &replay_input.segments[**right_index];
            let left_score = score_ready_segment(
                connection,
                &replay_input.semantic_defaults,
                args,
                left_segment,
                client_send_elapsed_ms,
            );
            let right_score = score_ready_segment(
                connection,
                &replay_input.semantic_defaults,
                args,
                right_segment,
                client_send_elapsed_ms,
            );

            left_score
                .total_cmp(&right_score)
                .then_with(|| right_segment.start_time_ms.cmp(&left_segment.start_time_ms))
                .then_with(|| right_segment.sequence.cmp(&left_segment.sequence))
        })
        .map(|(position, _)| position)
        .unwrap_or(0)
}

fn score_ready_segment(
    connection: &quinn::Connection,
    semantic_defaults: &ReplaySemanticDefaults,
    args: &Args,
    segment: &PreparedReplaySegment,
    client_send_elapsed_ms: u64,
) -> f64 {
    let profile = derive_runtime_utility_profile(
        args.mode,
        SegmentKind::Media,
        segment.sequence,
        semantic_defaults,
        segment.semantic_hint.as_ref(),
    );
    let traffic_class = match args.mode {
        ReplayMode::Vod => TrafficClass::Vod,
        ReplayMode::Live => TrafficClass::Live,
    };
    let queue_delay_ms = client_send_elapsed_ms.saturating_sub(segment.start_time_ms);
    let deadline_ms = compute_deadline_ms(
        args.mode,
        segment.start_time_ms,
        segment.duration_ms,
        args.vod_deadline_slack_ms,
    );
    let deadline_budget_ms = deadline_ms
        .saturating_sub(segment.start_time_ms)
        .max(segment.duration_ms);
    let freshness_window_ms = profile.freshness_window_ms.unwrap_or(match args.mode {
        ReplayMode::Vod => deadline_budget_ms,
        ReplayMode::Live => segment.duration_ms.max(1),
    });
    let dependency_ready = derive_dependency_ready(profile, queue_delay_ms, freshness_window_ms);
    let semantics = MediaSemantics::new(
        traffic_class,
        profile.importance,
        segment.payload.len() as u64,
    )
    .with_dependency_depth(profile.dependency_depth)
    .with_delivery_deadline(Duration::from_millis(deadline_budget_ms))
    .with_freshness_window(Duration::from_millis(freshness_window_ms));

    DefaultUtilityScorer
        .score(&UtilityInputs {
            semantics,
            queue_delay: Duration::from_millis(queue_delay_ms),
            estimated_rtt: connection.rtt(),
            dependency_ready,
        })
        .0
}

async fn send_segment(
    connection: &quinn::Connection,
    send: &mut quinn::SendStream,
    asset_name: &str,
    baseline_controller: BaselineController,
    mode: ReplayMode,
    kind: SegmentKind,
    sequence: u64,
    start_time_ms: u64,
    duration_ms: u64,
    deadline_ms: u64,
    client_send_elapsed_ms: u64,
    payload: &[u8],
    segment_path: &str,
    semantic_defaults: &ReplaySemanticDefaults,
    semantic_hint: Option<&ReplaySegmentSemanticHint>,
    runtime_utility: &RuntimeUtilityState,
) -> Result<()> {
    let runtime_utility = baseline_controller.uses_runtime_utility().then(|| {
        let profile =
            derive_runtime_utility_profile(mode, kind, sequence, semantic_defaults, semantic_hint);

        update_runtime_utility(
            connection,
            runtime_utility,
            mode,
            start_time_ms,
            duration_ms,
            deadline_ms,
            client_send_elapsed_ms,
            payload.len() as u64,
            profile,
        )
    });
    let header = SegmentHeader {
        asset_name: asset_name.to_string(),
        baseline_controller,
        mode,
        kind,
        sequence,
        start_time_ms,
        duration_ms,
        deadline_ms,
        client_send_elapsed_ms,
        payload_len: payload.len() as u64,
        segment_path: segment_path.to_string(),
        runtime_utility,
    };
    let header_bytes = bincode::serde::encode_to_vec(&header, bincode::config::standard())
        .context("failed to encode segment header")?;
    let header_len = u32::try_from(header_bytes.len()).context("segment header too large")?;
    let mut wire_header = Vec::with_capacity(4 + header_bytes.len());
    wire_header.extend_from_slice(&header_len.to_be_bytes());
    wire_header.extend_from_slice(&header_bytes);

    send.write_all(&wire_header)
        .await
        .context("failed to write header")?;
    send.write_all(&payload)
        .await
        .with_context(|| format!("failed to write payload for {}", segment_path))?;

    Ok(())
}

async fn load_manifest(path: &Path) -> Result<ReplayManifest> {
    let manifest_bytes = fs::read(path)
        .await
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_slice(&manifest_bytes).context("failed to parse replay manifest JSON")
}

pub async fn inspect_replay_input(path: &Path) -> Result<ReplayAssetInventory> {
    Ok(inspect_replay_input_impl(path).await?.inventory)
}

pub async fn prepare_replay_input(path: &Path) -> Result<PreparedReplayInput> {
    let inspection = inspect_replay_input_impl(path).await?;
    let manifest = inspection.manifest;
    let asset_root = inspection.asset_root;

    let init_path = asset_root.join(&manifest.init_segment);
    let init_payload = read_payload_bytes(&init_path, None, "init segment").await?;

    let mut seen_sequences = HashSet::with_capacity(manifest.segments.len());
    let mut previous_sequence = None;
    let mut previous_start_time_ms = None;
    let mut prepared_segments = Vec::with_capacity(manifest.segments.len());

    for segment in &manifest.segments {
        validate_replay_segment(
            segment,
            &mut seen_sequences,
            &mut previous_sequence,
            &mut previous_start_time_ms,
            &manifest.semantic_defaults,
        )?;

        let segment_path = asset_root.join(&segment.relative_path);
        let payload =
            read_payload_bytes(&segment_path, Some(segment.size_bytes), "segment payload")
                .await
                .with_context(|| {
                    format!("segment {} payload preflight failed", segment.sequence)
                })?;

        prepared_segments.push(PreparedReplaySegment {
            sequence: segment.sequence,
            relative_path: segment.relative_path.clone(),
            start_time_ms: segment.start_time_ms,
            duration_ms: segment.duration_ms,
            payload,
            semantic_hint: segment.semantic_hint.clone(),
        });
    }

    Ok(PreparedReplayInput {
        asset_name: manifest.asset_name,
        semantic_defaults: manifest.semantic_defaults,
        init_segment: PreparedInitSegment {
            relative_path: manifest.init_segment,
            payload: init_payload,
        },
        segments: prepared_segments,
    })
}

async fn inspect_replay_input_impl(path: &Path) -> Result<ReplayInspection> {
    let manifest = load_manifest(path).await?;
    validate_replay_input(&manifest)?;

    let asset_root = replay_asset_root(path, &manifest)?;
    let manifest_metadata = fs::metadata(path)
        .await
        .with_context(|| format!("failed to inspect replay manifest {}", path.display()))?;
    if !manifest_metadata.is_file() {
        return Err(anyhow!(
            "replay manifest path is not a file: {}",
            path.display()
        ));
    }
    let manifest_modified = manifest_metadata
        .modified()
        .with_context(|| format!("failed to read mtime for {}", path.display()))?;

    let init_segment_path = asset_root.join(&manifest.init_segment);
    let init_metadata = fs::metadata(&init_segment_path)
        .await
        .with_context(|| format!("missing init segment {}", init_segment_path.display()))?;
    if !init_metadata.is_file() {
        return Err(anyhow!(
            "init segment path is not a file: {}",
            init_segment_path.display()
        ));
    }
    if init_metadata.len() == 0 {
        return Err(anyhow!(
            "init segment is empty: {}",
            init_segment_path.display()
        ));
    }

    let mut seen_sequences = HashSet::with_capacity(manifest.segments.len());
    let mut previous_sequence = None;
    let mut previous_start_time_ms = None;
    let mut total_payload_bytes = 0u64;
    let mut segment_paths = Vec::with_capacity(manifest.segments.len());
    let mut newest_asset_path = init_segment_path.clone();
    let mut newest_asset_modified = init_metadata
        .modified()
        .with_context(|| format!("failed to read mtime for {}", init_segment_path.display()))?;

    for segment in &manifest.segments {
        validate_replay_segment(
            segment,
            &mut seen_sequences,
            &mut previous_sequence,
            &mut previous_start_time_ms,
            &manifest.semantic_defaults,
        )?;

        let segment_path = asset_root.join(&segment.relative_path);
        let segment_metadata = fs::metadata(&segment_path)
            .await
            .with_context(|| format!("missing segment payload {}", segment_path.display()))?;
        if !segment_metadata.is_file() {
            return Err(anyhow!(
                "segment payload path is not a file: {}",
                segment_path.display()
            ));
        }
        if segment_metadata.len() == 0 {
            return Err(anyhow!(
                "segment payload is empty: {}",
                segment_path.display()
            ));
        }
        if segment_metadata.len() != segment.size_bytes {
            return Err(anyhow!(
                "segment payload size mismatch for {}: manifest says {}, file has {}",
                segment_path.display(),
                segment.size_bytes,
                segment_metadata.len()
            ));
        }

        let segment_modified = segment_metadata
            .modified()
            .with_context(|| format!("failed to read mtime for {}", segment_path.display()))?;
        if segment_modified > newest_asset_modified {
            newest_asset_modified = segment_modified;
            newest_asset_path = segment_path.clone();
        }

        total_payload_bytes = total_payload_bytes.saturating_add(segment.size_bytes);
        segment_paths.push(segment_path);
    }

    if newest_asset_modified > manifest_modified {
        return Err(anyhow!(
            "replay manifest {} is older than asset payload {}; regenerate the replay manifest before running the suite",
            path.display(),
            newest_asset_path.display()
        ));
    }

    Ok(ReplayInspection {
        asset_root,
        inventory: ReplayAssetInventory {
            asset_name: manifest.asset_name.clone(),
            schema_version: replay_manifest_schema_version(&manifest)?,
            init_segment_path,
            segment_paths,
            total_payload_bytes,
        },
        manifest,
    })
}

fn validate_replay_input(manifest: &ReplayManifest) -> Result<()> {
    let _ = replay_manifest_schema_version(manifest)?;
    if manifest.asset_name.trim().is_empty() {
        return Err(anyhow!("replay manifest asset_name must not be empty"));
    }
    if manifest.init_segment.trim().is_empty() {
        return Err(anyhow!("replay manifest init_segment must not be empty"));
    }
    if manifest.segments.is_empty() {
        return Err(anyhow!(
            "replay manifest must contain at least one media segment"
        ));
    }

    if let Some(default_freshness_window_ms) =
        manifest.semantic_defaults.default_freshness_window_ms
    {
        if default_freshness_window_ms == 0 {
            return Err(anyhow!(
                "replay manifest semantic_defaults.default_freshness_window_ms must be greater than zero"
            ));
        }
    }

    validate_asset_relative_path(&manifest.init_segment, "init_segment")?;

    Ok(())
}

fn replay_manifest_schema_version(manifest: &ReplayManifest) -> Result<u32> {
    let schema_version = manifest
        .schema_version
        .unwrap_or(REPLAY_MANIFEST_SCHEMA_VERSION);
    if schema_version != REPLAY_MANIFEST_SCHEMA_VERSION {
        return Err(anyhow!(
            "unsupported replay manifest schema_version {} (expected {})",
            schema_version,
            REPLAY_MANIFEST_SCHEMA_VERSION
        ));
    }
    Ok(schema_version)
}

fn replay_asset_root(path: &Path, manifest: &ReplayManifest) -> Result<PathBuf> {
    Ok(path
        .parent()
        .context("replay manifest path must have a parent directory")?
        .join("..")
        .join("segments")
        .join(&manifest.asset_name))
}

fn validate_replay_segment(
    segment: &ReplaySegment,
    seen_sequences: &mut HashSet<u64>,
    previous_sequence: &mut Option<u64>,
    previous_start_time_ms: &mut Option<u64>,
    semantic_defaults: &ReplaySemanticDefaults,
) -> Result<()> {
    validate_asset_relative_path(&segment.relative_path, "segment relative_path")
        .with_context(|| format!("segment {} has an invalid relative_path", segment.sequence))?;
    if segment.duration_ms == 0 {
        return Err(anyhow!("segment {} has zero duration_ms", segment.sequence));
    }
    if segment.size_bytes == 0 {
        return Err(anyhow!("segment {} has zero size_bytes", segment.sequence));
    }
    if segment
        .start_time_ms
        .checked_add(segment.duration_ms)
        .is_none()
    {
        return Err(anyhow!(
            "segment {} start_time_ms + duration_ms overflows u64",
            segment.sequence
        ));
    }
    if !seen_sequences.insert(segment.sequence) {
        return Err(anyhow!(
            "duplicate segment sequence {} in replay manifest",
            segment.sequence
        ));
    }
    if let Some(previous) = *previous_sequence {
        if segment.sequence <= previous {
            return Err(anyhow!(
                "segment sequences must be strictly increasing: {} after {}",
                segment.sequence,
                previous
            ));
        }
    }
    if let Some(previous) = *previous_start_time_ms {
        if segment.start_time_ms < previous {
            return Err(anyhow!(
                "segment start_time_ms must be nondecreasing: {} after {}",
                segment.start_time_ms,
                previous
            ));
        }
    }
    if let Some(semantic_hint) = segment.semantic_hint.as_ref() {
        if let Some(freshness_window_ms) = semantic_hint.freshness_window_ms {
            if freshness_window_ms == 0 {
                return Err(anyhow!(
                    "segment {} has zero freshness_window_ms semantic hint",
                    segment.sequence
                ));
            }
        }
    }

    let profile = derive_runtime_utility_profile(
        ReplayMode::Vod,
        SegmentKind::Media,
        segment.sequence,
        semantic_defaults,
        segment.semantic_hint.as_ref(),
    );
    validate_runtime_utility_profile(segment.sequence, profile)?;

    *previous_sequence = Some(segment.sequence);
    *previous_start_time_ms = Some(segment.start_time_ms);
    Ok(())
}

fn compute_deadline_ms(
    mode: ReplayMode,
    start_time_ms: u64,
    duration_ms: u64,
    vod_deadline_slack_ms: u64,
) -> u64 {
    match mode {
        ReplayMode::Vod => start_time_ms
            .saturating_add(duration_ms)
            .saturating_add(vod_deadline_slack_ms),
        ReplayMode::Live => start_time_ms.saturating_add(duration_ms),
    }
}

fn update_runtime_utility(
    connection: &quinn::Connection,
    runtime_utility: &RuntimeUtilityState,
    mode: ReplayMode,
    start_time_ms: u64,
    duration_ms: u64,
    deadline_ms: u64,
    client_send_elapsed_ms: u64,
    payload_len: u64,
    profile: RuntimeUtilityProfile,
) -> RuntimeUtilityTelemetry {
    let traffic_class = match mode {
        ReplayMode::Vod => TrafficClass::Vod,
        ReplayMode::Live => TrafficClass::Live,
    };
    let queue_delay_ms = client_send_elapsed_ms.saturating_sub(start_time_ms);
    let estimated_rtt = connection.rtt();
    let mut semantics = MediaSemantics::new(traffic_class, profile.importance, payload_len)
        .with_dependency_depth(profile.dependency_depth);

    let deadline_budget_ms = deadline_ms.saturating_sub(start_time_ms).max(duration_ms);
    semantics = semantics.with_delivery_deadline(Duration::from_millis(deadline_budget_ms));

    let freshness_window_ms = profile.freshness_window_ms.unwrap_or(match mode {
        ReplayMode::Vod => deadline_budget_ms,
        ReplayMode::Live => duration_ms.max(1),
    });
    let dependency_ready = derive_dependency_ready(profile, queue_delay_ms, freshness_window_ms);
    semantics = semantics.with_freshness_window(Duration::from_millis(freshness_window_ms));

    let inputs = UtilityInputs {
        semantics: semantics.clone(),
        queue_delay: Duration::from_millis(queue_delay_ms),
        estimated_rtt,
        dependency_ready,
    };
    let observed_signal = UtilitySignal::from_score_for_traffic_class(
        traffic_class,
        DefaultUtilityScorer.score(&inputs),
    );
    let signal = runtime_utility.update_from_inputs(&DefaultUtilityScorer, &inputs);

    RuntimeUtilityTelemetry {
        traffic_class,
        importance: profile.importance,
        dependency_depth: profile.dependency_depth,
        dependency_ready,
        queue_delay_ms,
        estimated_rtt_ms: estimated_rtt.as_millis() as u64,
        utility_score: signal.score.0,
        observed_utility_score: Some(observed_signal.score.0),
        smoothed_utility_score: Some(signal.score.0),
        ewma_weight: Some(UTILITY_SIGNAL_EWMA_WEIGHT),
        ack_gain: signal.ack_gain,
        loss_reduction_factor: signal.loss_reduction_factor,
        controller_snapshot: runtime_utility
            .controller_snapshot()
            .map(amc_controller_snapshot_telemetry),
    }
}

fn amc_controller_snapshot_telemetry(
    snapshot: AmcControllerSnapshot,
) -> AmcControllerSnapshotTelemetry {
    let current_mtu_bytes = snapshot.current_mtu_bytes.max(1);

    AmcControllerSnapshotTelemetry {
        phase: amc_controller_phase_label(snapshot.phase).to_string(),
        last_event: amc_controller_event_label(snapshot.last_event).to_string(),
        current_mtu_bytes,
        congestion_window_bytes: snapshot.congestion_window_bytes,
        congestion_window_datagrams: snapshot.congestion_window_bytes.div_ceil(current_mtu_bytes),
        ssthresh_bytes: snapshot.ssthresh_bytes,
        ssthresh_datagrams: snapshot
            .ssthresh_bytes
            .map(|value| value.div_ceil(current_mtu_bytes)),
        initial_window_bytes: snapshot.initial_window_bytes,
        initial_window_datagrams: snapshot.initial_window_bytes.div_ceil(current_mtu_bytes),
        min_window_bytes: snapshot.min_window_bytes,
        min_window_datagrams: snapshot.min_window_bytes.div_ceil(current_mtu_bytes),
        max_window_bytes: snapshot.max_window_bytes,
        max_window_datagrams: snapshot.max_window_bytes.div_ceil(current_mtu_bytes),
        class_max_window_bytes: snapshot.class_max_window_bytes,
        class_max_window_datagrams: snapshot.class_max_window_bytes.div_ceil(current_mtu_bytes),
        growth_step_bytes: snapshot.growth_step_bytes,
        growth_step_datagrams: snapshot.growth_step_bytes.div_ceil(current_mtu_bytes),
    }
}

fn amc_controller_phase_label(phase: AmcControllerPhase) -> &'static str {
    match phase {
        AmcControllerPhase::SlowStart => "slow_start",
        AmcControllerPhase::CongestionAvoidance => "congestion_avoidance",
        AmcControllerPhase::Recovery => "recovery",
    }
}

fn amc_controller_event_label(event: AmcControllerEvent) -> &'static str {
    match event {
        AmcControllerEvent::Initialized => "initialized",
        AmcControllerEvent::Ack => "ack",
        AmcControllerEvent::Loss => "loss",
        AmcControllerEvent::PersistentCongestion => "persistent_congestion",
        AmcControllerEvent::MtuUpdate => "mtu_update",
    }
}

fn derive_runtime_utility_profile(
    mode: ReplayMode,
    kind: SegmentKind,
    sequence: u64,
    semantic_defaults: &ReplaySemanticDefaults,
    semantic_hint: Option<&ReplaySegmentSemanticHint>,
) -> RuntimeUtilityProfile {
    if matches!(kind, SegmentKind::Init) {
        return RuntimeUtilityProfile {
            importance: Importance::Critical,
            dependency_depth: 0,
            independent: true,
            freshness_window_ms: None,
        };
    }

    let startup_segment_count = semantic_defaults.startup_segment_count.unwrap_or(2);
    let importance = semantic_hint
        .and_then(|hint| hint.importance_hint)
        .unwrap_or_else(|| fallback_importance(mode, sequence, startup_segment_count));
    let dependency_depth = semantic_hint
        .and_then(|hint| hint.dependency_depth_hint)
        .or(semantic_defaults.default_dependency_depth_hint)
        .unwrap_or_else(|| fallback_dependency_depth(mode, sequence));
    let independent = semantic_hint
        .and_then(|hint| hint.independent)
        .unwrap_or(dependency_depth == 0);
    let freshness_window_ms = semantic_hint
        .and_then(|hint| hint.freshness_window_ms)
        .or(semantic_defaults.default_freshness_window_ms);

    RuntimeUtilityProfile {
        importance,
        dependency_depth,
        independent,
        freshness_window_ms,
    }
}

fn derive_dependency_ready(
    profile: RuntimeUtilityProfile,
    queue_delay_ms: u64,
    freshness_window_ms: u64,
) -> bool {
    profile.independent
        || profile.dependency_depth == 0
        || queue_delay_ms <= freshness_window_ms.max(1)
}

fn validate_runtime_utility_profile(sequence: u64, profile: RuntimeUtilityProfile) -> Result<()> {
    if profile.independent && profile.dependency_depth > 0 {
        return Err(anyhow!(
            "segment {} resolves to independent=true with dependency_depth_hint={} which is inconsistent",
            sequence,
            profile.dependency_depth
        ));
    }

    if !profile.independent && profile.dependency_depth == 0 {
        return Err(anyhow!(
            "segment {} resolves to independent=false with dependency_depth_hint=0 which is inconsistent",
            sequence
        ));
    }

    Ok(())
}

fn fallback_importance(mode: ReplayMode, sequence: u64, startup_segment_count: u64) -> Importance {
    if sequence <= startup_segment_count {
        return Importance::High;
    }

    match mode {
        ReplayMode::Vod => Importance::Normal,
        ReplayMode::Live => Importance::High,
    }
}

fn fallback_dependency_depth(mode: ReplayMode, sequence: u64) -> u8 {
    if sequence % 4 == 1 {
        return 0;
    }

    match mode {
        ReplayMode::Vod => 1,
        ReplayMode::Live => 2,
    }
}

fn build_client_config_from_cert_der(
    cert_der: &[u8],
    baseline_controller: BaselineController,
    runtime_utility: Arc<RuntimeUtilityState>,
) -> Result<ClientConfig> {
    let roots = load_root_cert_store_from_der(cert_der)?;

    let mut client_config = ClientConfig::with_root_certificates(Arc::new(roots))?;
    let mut transport_config = quinn::TransportConfig::default();
    transport_config.congestion_controller_factory(baseline_controller.factory(runtime_utility));
    client_config.transport_config(Arc::new(transport_config));

    Ok(client_config)
}

pub async fn certificate_is_ready(cert_path: &Path) -> Result<bool> {
    let cert_der = match fs::read(cert_path).await {
        Ok(cert_der) => cert_der,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", cert_path.display()));
        }
    };

    if cert_der.is_empty() {
        return Ok(false);
    }

    let mut roots = quinn::rustls::RootCertStore::empty();
    Ok(roots
        .add(quinn::rustls::pki_types::CertificateDer::from(cert_der))
        .is_ok())
}

fn load_root_cert_store_from_der(cert_der: &[u8]) -> Result<quinn::rustls::RootCertStore> {
    let mut roots = quinn::rustls::RootCertStore::empty();
    roots
        .add(quinn::rustls::pki_types::CertificateDer::from(
            cert_der.to_vec(),
        ))
        .context("failed to add server certificate to root store")?;

    Ok(roots)
}

fn validate_asset_relative_path(path: &str, field_name: &str) -> Result<()> {
    if path.trim().is_empty() {
        return Err(anyhow!("replay manifest {} must not be empty", field_name));
    }

    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return Err(anyhow!("replay manifest {} must be relative", field_name));
    }

    for component in candidate.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir => {
                return Err(anyhow!(
                    "replay manifest {} must not contain '.' path segments",
                    field_name
                ));
            }
            Component::ParentDir => {
                return Err(anyhow!(
                    "replay manifest {} must not escape the asset directory",
                    field_name
                ));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(anyhow!("replay manifest {} must be relative", field_name));
            }
        }
    }

    Ok(())
}

async fn read_payload_bytes(
    file_path: &Path,
    expected_size_bytes: Option<u64>,
    label: &str,
) -> Result<Vec<u8>> {
    let metadata = fs::metadata(file_path)
        .await
        .with_context(|| format!("missing {} {}", label, file_path.display()))?;
    if !metadata.is_file() {
        return Err(anyhow!(
            "{} path is not a file: {}",
            label,
            file_path.display()
        ));
    }

    let payload = fs::read(file_path)
        .await
        .with_context(|| format!("failed to read {} {}", label, file_path.display()))?;
    if payload.is_empty() {
        return Err(anyhow!("{} is empty: {}", label, file_path.display()));
    }

    if let Some(expected_size_bytes) = expected_size_bytes {
        if payload.len() as u64 != expected_size_bytes {
            return Err(anyhow!(
                "{} size mismatch for {}: manifest says {}, file has {}",
                label,
                file_path.display(),
                expected_size_bytes,
                payload.len()
            ));
        }
    }

    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::{
        AmcControllerSnapshotTelemetry, BaselineController, ReplayMode, RuntimeUtilityTelemetry,
        SegmentHeader, SegmentKind, inspect_replay_input,
    };
    use amc_core::{Importance, TrafficClass};
    use anyhow::Result;
    use serde_json::json;
    use std::{
        env, fs as stdfs,
        path::PathBuf,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    use tokio::fs;

    fn unique_temp_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        env::temp_dir().join(format!("quinn-amc-{label}-{}-{unique}", std::process::id()))
    }

    async fn write_replay_fixture(root: &PathBuf, manifest_size_bytes: u64) -> Result<PathBuf> {
        let manifests_dir = root.join("data/processed/manifests");
        let segments_dir = root.join("data/processed/segments/test_asset");
        fs::create_dir_all(&manifests_dir).await?;
        fs::create_dir_all(&segments_dir).await?;

        fs::write(segments_dir.join("test_asset_init.mp4"), b"init-bytes").await?;
        fs::write(
            segments_dir.join("test_asset_chunk_00001.m4s"),
            b"segment-bytes",
        )
        .await?;

        let manifest_path = manifests_dir.join("test_asset_replay.json");
        let manifest = json!({
            "schema_version": 1,
            "asset_name": "test_asset",
            "init_segment": "test_asset_init.mp4",
            "semantic_defaults": {
                "startup_segment_count": 1,
                "default_dependency_depth_hint": 1,
                "default_freshness_window_ms": 1000
            },
            "segments": [
                {
                    "sequence": 1,
                    "relative_path": "test_asset_chunk_00001.m4s",
                    "start_time_ms": 0,
                    "duration_ms": 1000,
                    "size_bytes": manifest_size_bytes,
                    "semantic_hint": {
                        "importance_hint": "critical",
                        "dependency_depth_hint": 0,
                        "independent": true,
                        "freshness_window_ms": 1000
                    }
                }
            ]
        });
        fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest)?).await?;
        Ok(manifest_path)
    }

    #[tokio::test]
    async fn inspect_replay_input_rejects_size_mismatch() -> Result<()> {
        let root = unique_temp_dir("manifest-size-mismatch");
        fs::create_dir_all(&root).await?;
        let manifest_path = write_replay_fixture(&root, 999).await?;

        let error = inspect_replay_input(&manifest_path).await.unwrap_err();
        assert!(error.to_string().contains("segment payload size mismatch"));

        stdfs::remove_dir_all(&root)?;
        Ok(())
    }

    #[tokio::test]
    async fn inspect_replay_input_rejects_stale_manifest() -> Result<()> {
        let root = unique_temp_dir("manifest-stale");
        fs::create_dir_all(&root).await?;
        let manifest_path = write_replay_fixture(&root, b"segment-bytes".len() as u64).await?;

        tokio::time::sleep(Duration::from_millis(1100)).await;
        let segment_path =
            root.join("data/processed/segments/test_asset/test_asset_chunk_00001.m4s");
        fs::write(&segment_path, b"segment-bytes").await?;

        let error = inspect_replay_input(&manifest_path).await.unwrap_err();
        assert!(error.to_string().contains("older than asset payload"));

        stdfs::remove_dir_all(&root)?;
        Ok(())
    }

    #[test]
    fn segment_header_bincode_round_trip_preserves_amc_snapshot_telemetry() {
        let header = SegmentHeader {
            asset_name: "test_asset".to_string(),
            baseline_controller: BaselineController::AmcPreview,
            mode: ReplayMode::Live,
            kind: SegmentKind::Media,
            sequence: 7,
            start_time_ms: 120,
            duration_ms: 1_000,
            deadline_ms: 900,
            client_send_elapsed_ms: 123,
            payload_len: 4_096,
            segment_path: "test_asset_chunk_00007.m4s".to_string(),
            runtime_utility: Some(RuntimeUtilityTelemetry {
                traffic_class: TrafficClass::Live,
                importance: Importance::High,
                dependency_depth: 1,
                dependency_ready: true,
                queue_delay_ms: 12,
                estimated_rtt_ms: 44,
                utility_score: 0.01,
                observed_utility_score: Some(0.012),
                smoothed_utility_score: Some(0.01),
                ewma_weight: Some(0.35),
                ack_gain: 1.2,
                loss_reduction_factor: 0.65,
                controller_snapshot: Some(AmcControllerSnapshotTelemetry {
                    phase: "congestion_avoidance".to_string(),
                    last_event: "ack".to_string(),
                    current_mtu_bytes: 1_200,
                    congestion_window_bytes: 48_000,
                    congestion_window_datagrams: 40,
                    ssthresh_bytes: Some(24_000),
                    ssthresh_datagrams: Some(20),
                    initial_window_bytes: 24_000,
                    initial_window_datagrams: 20,
                    min_window_bytes: 4_800,
                    min_window_datagrams: 4,
                    max_window_bytes: 480_000,
                    max_window_datagrams: 400,
                    class_max_window_bytes: 192_000,
                    class_max_window_datagrams: 160,
                    growth_step_bytes: 1_200,
                    growth_step_datagrams: 1,
                }),
            }),
        };

        let encoded = bincode::serde::encode_to_vec(&header, bincode::config::standard())
            .expect("segment header encode");
        let (decoded, _): (SegmentHeader, usize) =
            bincode::serde::decode_from_slice(&encoded, bincode::config::standard())
                .expect("segment header decode");

        let runtime = decoded.runtime_utility.expect("runtime utility");
        let snapshot = runtime.controller_snapshot.expect("controller snapshot");
        assert_eq!(runtime.observed_utility_score, Some(0.012));
        assert_eq!(runtime.smoothed_utility_score, Some(0.01));
        assert_eq!(runtime.ewma_weight, Some(0.35));
        assert_eq!(snapshot.phase, "congestion_avoidance");
        assert_eq!(snapshot.last_event, "ack");
        assert_eq!(snapshot.congestion_window_datagrams, 40);
        assert_eq!(snapshot.ssthresh_datagrams, Some(20));
    }
}
