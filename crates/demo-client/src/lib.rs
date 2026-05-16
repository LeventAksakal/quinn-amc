use std::{
    collections::HashSet,
    net::SocketAddr,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use amc_core::{
    AmcControllerConfig, DefaultUtilityScorer, Importance, MediaSemantics, RuntimeUtilityState,
    TrafficClass, UtilityInputs,
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

#[derive(Clone, Debug, Deserialize)]
struct ReplayManifest {
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
    pub ack_gain: f64,
    pub loss_reduction_factor: f64,
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
    let client_config = build_client_config_from_cert_der(
        cert_der,
        args.controller,
        runtime_utility.clone(),
    )?;

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
    for segment in &replay_input.segments {
        if matches!(args.pace, Pace::Realtime) {
            let deadline = tokio::time::Instant::from_std(
                replay_start + Duration::from_millis(segment.start_time_ms),
            );
            sleep_until(deadline).await;
        }

        send_segment(
            &connection,
            &mut send,
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
            runtime_utility.as_ref(),
        )
        .await?;
    }

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

    send.write_all(&header_len.to_be_bytes())
        .await
        .context("failed to write header length")?;
    send.write_all(&header_bytes)
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

pub async fn prepare_replay_input(path: &Path) -> Result<PreparedReplayInput> {
    let manifest = load_manifest(path).await?;
    let asset_root = path
        .parent()
        .context("replay manifest path must have a parent directory")?
        .join("..")
        .join("segments")
        .join(&manifest.asset_name);

    validate_replay_input(&manifest)?;

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
        let payload = read_payload_bytes(&segment_path, Some(segment.size_bytes), "segment payload")
            .await
            .with_context(|| format!("segment {} payload preflight failed", segment.sequence))?;

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

fn validate_replay_input(manifest: &ReplayManifest) -> Result<()> {
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

    let signal = runtime_utility.update_from_inputs(
        &DefaultUtilityScorer,
        &UtilityInputs {
            semantics,
            queue_delay: Duration::from_millis(queue_delay_ms),
            estimated_rtt,
            dependency_ready,
        },
    );

    RuntimeUtilityTelemetry {
        traffic_class,
        importance: profile.importance,
        dependency_depth: profile.dependency_depth,
        dependency_ready,
        queue_delay_ms,
        estimated_rtt_ms: estimated_rtt.as_millis() as u64,
        utility_score: signal.score.0,
        ack_gain: signal.ack_gain,
        loss_reduction_factor: signal.loss_reduction_factor,
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
        .add(quinn::rustls::pki_types::CertificateDer::from(cert_der.to_vec()))
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
