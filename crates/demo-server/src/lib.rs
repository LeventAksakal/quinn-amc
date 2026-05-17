use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use demo_client::{BaselineController, RuntimeUtilityTelemetry};
use quinn::{Endpoint, ServerConfig};
use rcgen::generate_simple_self_signed;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::{debug, info};

const REPORT_KIND: &str = "demo_server_transfer_report";
const REPORT_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentKind {
    Init,
    Media,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<ReportSchemaDescriptor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transfer_id: Option<String>,
    pub generated_at_unix_ms: u64,
    pub generated_by: ReportGenerator,
    pub server: ServerReportProvenance,
    pub connection: ConnectionReportProvenance,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transfer: Option<TransferReportIdentity>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReportSchemaDescriptor {
    pub format: String,
    pub compatibility: String,
    pub top_level_fields: Vec<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_directory: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cert_path_absolute: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub report_path_absolute: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConnectionReportProvenance {
    pub remote_address: String,
    pub transfer_started_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_address: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TransferReportIdentity {
    pub asset_name: String,
    pub baseline_controller: BaselineController,
    pub mode: ReplayMode,
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

pub struct SuiteServer {
    endpoint: Endpoint,
    bind: SocketAddr,
    cert_out: PathBuf,
    cert_der: Vec<u8>,
}

impl SuiteServer {
    pub async fn bind(bind: SocketAddr, cert_out: PathBuf) -> Result<Self> {
        let (server_config, cert_der) = build_server_config()?;
        let endpoint = Endpoint::server(server_config, bind)
            .with_context(|| format!("failed to bind server endpoint on {}", bind))?;
        write_cert(&cert_out, &cert_der).await?;

        info!(bind = %bind, cert = %cert_out.display(), "server ready");

        Ok(Self {
            endpoint,
            bind,
            cert_out,
            cert_der,
        })
    }

    pub fn cert_der(&self) -> &[u8] {
        &self.cert_der
    }

    pub async fn run_transfer(&self, report_out: &Path) -> Result<TransferReport> {
        let incoming = self
            .endpoint
            .accept()
            .await
            .ok_or_else(|| anyhow!("endpoint closed before receiving a connection"))?;
        let connection = incoming
            .await
            .context("failed to establish incoming connection")?;
        let remote = connection.remote_address();

        info!(remote = %remote, "connection established");

        process_transfer(&self.bind, &self.cert_out, report_out, remote, connection).await
    }

    pub async fn shutdown(&self) {
        self.endpoint.close(0u32.into(), b"suite complete");
        self.endpoint.wait_idle().await;
    }
}

pub async fn run(args: Args) -> Result<TransferSummary> {
    let suite_server = SuiteServer::bind(args.bind, args.cert_out.clone()).await?;
    let report = suite_server.run_transfer(&args.report_out).await?;
    suite_server.shutdown().await;
    Ok(report.summary)
}

async fn process_transfer(
    bind: &SocketAddr,
    cert_out: &Path,
    report_out: &Path,
    remote: SocketAddr,
    connection: quinn::Connection,
) -> Result<TransferReport> {
    let (mut send, mut recv) = connection
        .accept_bi()
        .await
        .context("failed to accept stream")?;
    let transfer_started_at_unix_ms = unix_time_ms(SystemTime::now())?;
    let transfer_start = Instant::now();

    let mut asset_name = String::from("unknown");
    let mut baseline_controller = None;
    let mut replay_mode = None;
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
        let (header, _): (SegmentHeader, usize) =
            bincode::serde::decode_from_slice(&header_bytes, bincode::config::standard())
                .context("failed to decode segment header")?;

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

        if let Some(current_mode) = replay_mode {
            if current_mode != header.mode {
                return Err(anyhow!(
                    "received mixed replay modes on one connection: {:?} then {:?}",
                    current_mode,
                    header.mode
                ));
            }
        } else {
            replay_mode = Some(header.mode);
        }

        if segments_received > 0 && asset_name != header.asset_name {
            return Err(anyhow!(
                "received mixed asset names on one connection: {} then {}",
                asset_name,
                header.asset_name
            ));
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

        debug!(
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
        report_path: report_out.display().to_string(),
    };
    let transfer_identity = match (baseline_controller, replay_mode) {
        (Some(baseline_controller), Some(mode)) => Some(TransferReportIdentity {
            asset_name: response.asset_name.clone(),
            baseline_controller,
            mode,
        }),
        _ => None,
    };
    let report = TransferReport {
        metadata: Some(build_report_metadata(
            *bind,
            cert_out,
            report_out,
            remote,
            transfer_started_at_unix_ms,
            transfer_identity,
        )?),
        summary: response.clone(),
        observations,
    };
    write_report(report_out, &report).await?;
    let response_bytes = serde_json::to_vec(&response).context("failed to encode response")?;
    send.write_all(&response_bytes)
        .await
        .context("failed to write response")?;
    send.finish().context("failed to finish response stream")?;
    send.stopped()
        .await
        .context("failed while waiting for response delivery")?;

    info!(
        remote = %remote,
        segments_received,
        media_segments_received,
        total_payload_bytes,
        useful_media_segments,
        late_media_segments,
        max_observed_lateness_ms,
        report_path = %report_out.display(),
        "transfer summary sent"
    );

    Ok(report)
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

async fn write_cert(path: &Path, cert_der: &[u8]) -> Result<()> {
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

async fn write_report(path: &Path, report: &TransferReport) -> Result<()> {
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

fn build_report_metadata(
    bind: SocketAddr,
    cert_out: &Path,
    report_out: &Path,
    remote: SocketAddr,
    transfer_started_at_unix_ms: u64,
    transfer: Option<TransferReportIdentity>,
) -> Result<ReportMetadata> {
    let process_id = std::process::id();
    Ok(ReportMetadata {
        report_kind: REPORT_KIND.to_string(),
        schema_version: REPORT_SCHEMA_VERSION,
        schema: Some(ReportSchemaDescriptor {
            format: "json".to_string(),
            compatibility: "backward_compatible_additive".to_string(),
            top_level_fields: vec![
                "metadata".to_string(),
                "summary".to_string(),
                "observations".to_string(),
            ],
        }),
        transfer_id: Some(build_transfer_id(
            remote,
            transfer_started_at_unix_ms,
            process_id,
        )),
        generated_at_unix_ms: unix_time_ms(SystemTime::now())?,
        generated_by: ReportGenerator {
            crate_name: env!("CARGO_PKG_NAME").to_string(),
            crate_version: env!("CARGO_PKG_VERSION").to_string(),
        },
        server: ServerReportProvenance {
            bind_address: bind.to_string(),
            cert_path: cert_out.display().to_string(),
            report_path: report_out.display().to_string(),
            process_id,
            host_name: host_name(),
            working_directory: working_directory(),
            cert_path_absolute: absolute_path_string(cert_out),
            report_path_absolute: absolute_path_string(report_out),
        },
        connection: ConnectionReportProvenance {
            remote_address: remote.to_string(),
            transfer_started_at_unix_ms,
            local_address: Some(bind.to_string()),
        },
        transfer,
    })
}

fn build_transfer_id(
    remote: SocketAddr,
    transfer_started_at_unix_ms: u64,
    process_id: u32,
) -> String {
    format!(
        "{}-{}-{}-{}",
        REPORT_KIND,
        transfer_started_at_unix_ms,
        process_id,
        sanitize_identifier_fragment(&remote.to_string())
    )
}

fn sanitize_identifier_fragment(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

fn host_name() -> Option<String> {
    ["HOSTNAME", "COMPUTERNAME"]
        .into_iter()
        .find_map(|key| std::env::var(key).ok())
        .filter(|value| !value.trim().is_empty())
}

fn working_directory() -> Option<String> {
    std::env::current_dir()
        .ok()
        .map(|path| path.display().to_string())
}

fn absolute_path_string(path: &Path) -> Option<String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    Some(absolute.display().to_string())
}

fn unix_time_ms(time: SystemTime) -> Result<u64> {
    let duration = time
        .duration_since(UNIX_EPOCH)
        .context("system clock is before unix epoch")?;
    Ok(duration.as_millis() as u64)
}
