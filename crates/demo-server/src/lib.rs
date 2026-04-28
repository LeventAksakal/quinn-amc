use std::{
    net::SocketAddr,
    path::PathBuf,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use demo_client::{BaselineController, RuntimeUtilityTelemetry};
use quinn::{Endpoint, ServerConfig};
use rcgen::generate_simple_self_signed;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::info;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentKind {
    Init,
    Media,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayMode {
    Vod,
    Live,
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
pub struct SegmentObservation {
    pub asset_name: String,
    pub mode: ReplayMode,
    pub kind: SegmentKind,
    pub sequence: u64,
    pub start_time_ms: u64,
    pub duration_ms: u64,
    pub deadline_ms: u64,
    pub client_send_elapsed_ms: u64,
    pub server_receive_elapsed_ms: u64,
    pub payload_len: u64,
    pub segment_path: String,
    pub lateness_ms: i64,
    pub useful: bool,
    pub runtime_utility: Option<RuntimeUtilityTelemetry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TransferReport {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ReportMetadata>,
    pub summary: TransferSummary,
    pub observations: Vec<SegmentObservation>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReportMetadata {
    pub report_kind: String,
    pub schema_version: u32,
    pub generated_at_unix_ms: u64,
    pub generated_by: ReportGenerator,
    pub server: ServerReportProvenance,
    pub connection: ConnectionReportProvenance,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReportGenerator {
    pub crate_name: String,
    pub crate_version: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ServerReportProvenance {
    pub bind_address: String,
    pub cert_path: String,
    pub report_path: String,
    pub process_id: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConnectionReportProvenance {
    pub remote_address: String,
    pub transfer_started_at_unix_ms: u64,
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
    #[arg(long, default_value = "127.0.0.1:5000")]
    pub bind: SocketAddr,

    #[arg(long, default_value = "demo-cert.der")]
    pub cert_out: PathBuf,

    #[arg(long, default_value = "results/raw/demo/latest_transfer_report.json")]
    pub report_out: PathBuf,
}

pub async fn run(args: Args) -> Result<TransferSummary> {
    let (server_config, cert_der) = build_server_config()?;

    let endpoint = Endpoint::server(server_config, args.bind)
        .with_context(|| format!("failed to bind server endpoint on {}", args.bind))?;
    write_cert(&args.cert_out, &cert_der).await?;

    info!(bind = %args.bind, cert = %args.cert_out.display(), "server ready");

    let incoming = endpoint
        .accept()
        .await
        .ok_or_else(|| anyhow!("endpoint closed before receiving a connection"))?;
    let connection = incoming
        .await
        .context("failed to establish incoming connection")?;
    let remote = connection.remote_address();

    info!(remote = %remote, "connection established");

    let (mut send, mut recv) = connection
        .accept_bi()
        .await
        .context("failed to accept stream")?;
    let transfer_started_at_unix_ms = unix_time_ms(SystemTime::now())?;
    let transfer_start = Instant::now();

    let mut asset_name = String::from("unknown");
    let mut baseline_controller = None;
    let mut segments_received = 0usize;
    let mut media_segments_received = 0usize;
    let mut total_payload_bytes = 0u64;
    let mut useful_media_segments = 0usize;
    let mut late_media_segments = 0usize;
    let mut max_observed_lateness_ms = i64::MIN;
    let mut amc_runtime_samples = 0usize;
    let mut max_runtime_utility_score = None;
    let mut min_runtime_utility_score = None;
    let mut observations = Vec::new();

    loop {
        let mut header_len_buf = [0u8; 4];
        match recv.read_exact(&mut header_len_buf).await {
            Ok(_) => {}
            Err(quinn::ReadExactError::FinishedEarly(0)) => break,
            Err(error) => return Err(error).context("failed to read header length"),
        }

        let header_len = u32::from_be_bytes(header_len_buf) as usize;
        let mut header_bytes = vec![0u8; header_len];
        recv.read_exact(&mut header_bytes)
            .await
            .context("failed to read segment header")?;
        let header: SegmentHeader =
            serde_json::from_slice(&header_bytes).context("failed to decode segment header")?;

        if let Some(current_controller) = baseline_controller {
            if current_controller != header.baseline_controller {
                return Err(anyhow!(
                    "received mixed baseline controllers on one connection: {:?} then {:?}",
                    current_controller,
                    header.baseline_controller
                ));
            }
        } else {
            baseline_controller = Some(header.baseline_controller);
        }

        let mut payload = vec![0u8; header.payload_len as usize];
        recv.read_exact(&mut payload)
            .await
            .with_context(|| format!("failed to read payload for {}", header.segment_path))?;

        if let Some(runtime_utility) = header.runtime_utility.as_ref() {
            amc_runtime_samples += 1;
            max_runtime_utility_score = Some(
                max_runtime_utility_score.map_or(runtime_utility.utility_score, |score: f64| {
                    score.max(runtime_utility.utility_score)
                }),
            );
            min_runtime_utility_score = Some(
                min_runtime_utility_score.map_or(runtime_utility.utility_score, |score: f64| {
                    score.min(runtime_utility.utility_score)
                }),
            );
        }

        asset_name = header.asset_name.clone();
        segments_received += 1;
        total_payload_bytes += header.payload_len;
        let server_receive_elapsed_ms = transfer_start.elapsed().as_millis() as u64;
        let lateness_ms = server_receive_elapsed_ms as i64 - header.deadline_ms as i64;
        let useful = !matches!(header.kind, SegmentKind::Media)
            || server_receive_elapsed_ms <= header.deadline_ms;
        if matches!(header.kind, SegmentKind::Media) {
            media_segments_received += 1;
            if useful {
                useful_media_segments += 1;
            } else {
                late_media_segments += 1;
            }
            max_observed_lateness_ms = max_observed_lateness_ms.max(lateness_ms.max(0));
        }

        observations.push(SegmentObservation {
            asset_name: header.asset_name.clone(),
            mode: header.mode,
            kind: header.kind,
            sequence: header.sequence,
            start_time_ms: header.start_time_ms,
            duration_ms: header.duration_ms,
            deadline_ms: header.deadline_ms,
            client_send_elapsed_ms: header.client_send_elapsed_ms,
            server_receive_elapsed_ms,
            payload_len: header.payload_len,
            segment_path: header.segment_path.clone(),
            lateness_ms,
            useful,
            runtime_utility: header.runtime_utility.clone(),
        });

        info!(
            remote = %remote,
            asset = %header.asset_name,
            mode = ?header.mode,
            kind = ?header.kind,
            sequence = header.sequence,
            start_time_ms = header.start_time_ms,
            duration_ms = header.duration_ms,
            deadline_ms = header.deadline_ms,
            client_send_elapsed_ms = header.client_send_elapsed_ms,
            server_receive_elapsed_ms,
            payload_len = header.payload_len,
            lateness_ms,
            useful,
            segment_path = %header.segment_path,
            "received segment"
        );
    }

    if max_observed_lateness_ms == i64::MIN {
        max_observed_lateness_ms = 0;
    }

    let response = TransferSummary {
        asset_name,
        baseline_controller: baseline_controller.unwrap_or_default(),
        segments_received,
        media_segments_received,
        total_payload_bytes,
        useful_media_segments,
        late_media_segments,
        max_observed_lateness_ms,
        amc_runtime_samples,
        max_runtime_utility_score,
        min_runtime_utility_score,
        report_path: args.report_out.display().to_string(),
    };
    write_report(
        &args.report_out,
        &TransferReport {
            metadata: Some(ReportMetadata {
                report_kind: "demo_server_transfer_report".to_string(),
                schema_version: 1,
                generated_at_unix_ms: unix_time_ms(SystemTime::now())?,
                generated_by: ReportGenerator {
                    crate_name: env!("CARGO_PKG_NAME").to_string(),
                    crate_version: env!("CARGO_PKG_VERSION").to_string(),
                },
                server: ServerReportProvenance {
                    bind_address: args.bind.to_string(),
                    cert_path: args.cert_out.display().to_string(),
                    report_path: args.report_out.display().to_string(),
                    process_id: std::process::id(),
                },
                connection: ConnectionReportProvenance {
                    remote_address: remote.to_string(),
                    transfer_started_at_unix_ms,
                },
            }),
            summary: response.clone(),
            observations,
        },
    )
    .await?;
    let response_bytes = serde_json::to_vec(&response).context("failed to encode response")?;
    send.write_all(&response_bytes)
        .await
        .context("failed to write response")?;
    send.finish().context("failed to finish response stream")?;

    info!(
        remote = %remote,
        segments_received,
        media_segments_received,
        total_payload_bytes,
        useful_media_segments,
        late_media_segments,
        max_observed_lateness_ms,
        report_path = %args.report_out.display(),
        "transfer summary sent"
    );
    endpoint.wait_idle().await;
    Ok(response)
}

fn build_server_config() -> Result<(ServerConfig, Vec<u8>)> {
    let certified_key = generate_simple_self_signed(vec!["localhost".to_string()])
        .context("failed to generate self-signed certificate")?;
    let cert_der = certified_key.cert.der().to_vec();
    let key_der = certified_key.signing_key.serialize_der();

    let server_config = ServerConfig::with_single_cert(
        vec![certified_key.cert.der().clone()],
        quinn::rustls::pki_types::PrivatePkcs8KeyDer::from(key_der).into(),
    )
    .context("failed to build server TLS configuration")?;

    Ok((server_config, cert_der))
}

async fn write_cert(path: &PathBuf, cert_der: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .await
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
    }

    fs::write(path, cert_der)
        .await
        .with_context(|| format!("failed to write certificate to {}", path.display()))
}

async fn write_report(path: &PathBuf, report: &TransferReport) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .await
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
    }

    let report_bytes =
        serde_json::to_vec_pretty(report).context("failed to encode transfer report")?;
    fs::write(path, report_bytes)
        .await
        .with_context(|| format!("failed to write transfer report to {}", path.display()))
}

fn unix_time_ms(time: SystemTime) -> Result<u64> {
    let duration = time
        .duration_since(UNIX_EPOCH)
        .context("system clock is before unix epoch")?;
    Ok(duration.as_millis() as u64)
}
