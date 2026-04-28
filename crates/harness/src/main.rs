mod analysis;
mod config;
mod network;

use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
};

use analysis::{
    RunOutcome, SuiteSummary, analyze_report, build_suite_comparison_export,
    load_replay_manifest, load_transfer_report, write_amc_analysis, write_comparison_export,
};
use anyhow::{Context, Result, anyhow};
use clap::Parser;
use config::{SuiteConfig, find_network_scenario, validate_suite_config};
use demo_client::{Args as ClientArgs, certificate_is_ready};
use demo_server::{Args as ServerArgs, TransferSummary};
use network::{apply_network_scenario, validate_network_scenario_for_run};
use tokio::{
    fs,
    task::JoinHandle,
    time::{Duration as TokioDuration, Instant as TokioInstant, sleep},
};
use tracing::info;

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, clap::Subcommand)]
enum Command {
    RunSuite {
        #[arg(long, default_value = "configs/harness/demo_vod_live.json")]
        config: PathBuf,
    },
    AnalyzeSuite {
        #[arg(long, default_value = "configs/harness/demo_vod_live.json")]
        config: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = tracing_subscriber::fmt().with_target(false).try_init();

    let args = Cli::parse();
    let workspace_root = workspace_root()?;
    match args.command {
        Command::RunSuite { config } => {
            let config_path = resolve_path(&workspace_root, &config);
            let config = load_config(&config_path).await?;
            preflight_suite(&workspace_root, &config, SuiteCommand::Run).await?;
            let summary_path = run_local_suite(&workspace_root, &config).await?;
            info!(summary_path = %path_relative_to(&workspace_root, &summary_path), "harness suite completed");
        }
        Command::AnalyzeSuite { config } => {
            let config_path = resolve_path(&workspace_root, &config);
            let config = load_config(&config_path).await?;
            preflight_suite(&workspace_root, &config, SuiteCommand::Analyze).await?;
            let summary_path = analyze_existing_suite(&workspace_root, &config).await?;
            info!(summary_path = %path_relative_to(&workspace_root, &summary_path), "harness analysis completed");
        }
    }

    Ok(())
}

async fn run_local_suite(workspace_root: &Path, config: &SuiteConfig) -> Result<PathBuf> {
    let replay_manifest_path = resolve_path(workspace_root, &config.replay_manifest);
    let replay_manifest = load_replay_manifest(&replay_manifest_path).await?;
    let mut runs = Vec::with_capacity(config.runs.len());

    for (index, run) in config.runs.iter().enumerate() {
        let network_scenario =
            find_network_scenario(&config.network_scenarios, &run.network_scenario)
                .ok_or_else(|| anyhow!("unknown network scenario {}", run.network_scenario))?;
        let port = config.base_port + index as u16;
        let server_addr: SocketAddr = format!("{}:{}", config.host, port)
            .parse()
            .with_context(|| format!("invalid host/port {}:{}", config.host, port))?;
        let cert_path = resolve_path(workspace_root, &config.cert_path);
        let absolute_report_path = report_output_path(workspace_root, config, &run.name);
        let amc_analysis_path = amc_output_path(workspace_root, config, &run.name);
        let _network_guard = apply_network_scenario(network_scenario)?;

        info!(run = %run.name, mode = ?run.mode, pace = ?run.pace, scenario = %network_scenario.name, server = %server_addr, "starting harness run");
        remove_file_if_exists(&cert_path).await?;
        let server_task = spawn_server(ServerArgs {
            bind: server_addr,
            cert_out: cert_path.clone(),
            report_out: absolute_report_path.clone(),
        });

        wait_for_server_certificate(&cert_path, config.server_startup_delay_ms).await?;

        let client_summary = demo_client::run(ClientArgs {
            bind: "0.0.0.0:0".parse().unwrap(),
            server: server_addr,
            server_name: "localhost".to_string(),
            cert: cert_path,
            replay_manifest: replay_manifest_path.clone(),
            pace: run.pace,
            controller: run.controller,
            mode: run.mode,
            vod_deadline_slack_ms: run.vod_deadline_slack_ms.unwrap_or(30_000),
        })
        .await
        .with_context(|| format!("client run {} failed", run.name))?;

        let server_summary = server_task
            .await
            .context("server task join failed")?
            .with_context(|| format!("server run {} failed", run.name))?;

        if client_summary.report_path != server_summary.report_path {
            return Err(anyhow!(
                "client/server report path mismatch for {}: {} vs {}",
                run.name,
                client_summary.report_path,
                server_summary.report_path
            ));
        }

        let transfer_report = load_transfer_report(&absolute_report_path).await?;
        if transfer_report.summary.baseline_controller != run.controller {
            return Err(anyhow!(
                "controller mismatch for {}: run config {:?} vs raw report {:?}",
                run.name,
                run.controller,
                transfer_report.summary.baseline_controller
            ));
        }
        let amc_analysis = analyze_report(
            run,
            network_scenario,
            &config.semantic_profile,
            &replay_manifest,
            &transfer_report,
        );
        write_amc_analysis(&amc_analysis_path, &amc_analysis).await?;

        runs.push(RunOutcome {
            name: run.name.clone(),
            controller: run.controller,
            mode: run.mode,
            pace: run.pace,
            server: server_addr.to_string(),
            report_path: path_relative_to(workspace_root, &absolute_report_path),
            network_scenario: network_scenario.clone(),
            amc_analysis_path: path_relative_to(workspace_root, &amc_analysis_path),
            amc_aggregate: amc_analysis.aggregate.clone(),
            summary: server_summary,
        });
    }

    write_suite_summary(workspace_root, config, runs).await
}

async fn analyze_existing_suite(workspace_root: &Path, config: &SuiteConfig) -> Result<PathBuf> {
    let replay_manifest_path = resolve_path(workspace_root, &config.replay_manifest);
    let replay_manifest = load_replay_manifest(&replay_manifest_path).await?;
    let mut runs = Vec::with_capacity(config.runs.len());

    for (index, run) in config.runs.iter().enumerate() {
        let network_scenario =
            find_network_scenario(&config.network_scenarios, &run.network_scenario)
                .ok_or_else(|| anyhow!("unknown network scenario {}", run.network_scenario))?;
        let port = config.base_port + index as u16;
        let server_addr = format!("{}:{}", config.host, port);
        let absolute_report_path = report_output_path(workspace_root, config, &run.name);
        let amc_analysis_path = amc_output_path(workspace_root, config, &run.name);
        let transfer_report = load_transfer_report(&absolute_report_path)
            .await
            .with_context(|| format!("missing raw report for run {}", run.name))?;
        if transfer_report.summary.baseline_controller != run.controller {
            return Err(anyhow!(
                "controller mismatch for {}: run config {:?} vs raw report {:?}",
                run.name,
                run.controller,
                transfer_report.summary.baseline_controller
            ));
        }
        let amc_analysis = analyze_report(
            run,
            network_scenario,
            &config.semantic_profile,
            &replay_manifest,
            &transfer_report,
        );
        write_amc_analysis(&amc_analysis_path, &amc_analysis).await?;

        runs.push(RunOutcome {
            name: run.name.clone(),
            controller: run.controller,
            mode: run.mode,
            pace: run.pace,
            server: server_addr,
            report_path: path_relative_to(workspace_root, &absolute_report_path),
            network_scenario: network_scenario.clone(),
            amc_analysis_path: path_relative_to(workspace_root, &amc_analysis_path),
            amc_aggregate: amc_analysis.aggregate.clone(),
            summary: transfer_report.summary,
        });
    }

    write_suite_summary(workspace_root, config, runs).await
}

async fn write_suite_summary(
    workspace_root: &Path,
    config: &SuiteConfig,
    runs: Vec<RunOutcome>,
) -> Result<PathBuf> {
    let summary = SuiteSummary {
        suite_name: config.suite_name.clone(),
        replay_manifest: path_relative_to(
            workspace_root,
            &resolve_path(workspace_root, &config.replay_manifest),
        ),
        network_scenarios: config.network_scenarios.clone(),
        runs,
    };
    let summary_path = summary_output_path(workspace_root, config);
    write_summary(&summary_path, &summary).await?;
    let comparison_path = comparison_output_path(workspace_root, config);
    let comparison_export = build_suite_comparison_export(
        &summary.suite_name,
        &summary.replay_manifest,
        &summary.runs,
    );
    write_comparison_export(&comparison_path, &comparison_export).await?;
    Ok(summary_path)
}

fn spawn_server(args: ServerArgs) -> JoinHandle<Result<TransferSummary>> {
    tokio::spawn(async move { demo_server::run(args).await })
}

fn report_output_path(workspace_root: &Path, config: &SuiteConfig, run_name: &str) -> PathBuf {
    resolve_path(
        workspace_root,
        &config
            .results_root
            .join("raw")
            .join("harness")
            .join(format!("{}_report.json", run_name)),
    )
}

fn amc_output_path(workspace_root: &Path, config: &SuiteConfig, run_name: &str) -> PathBuf {
    resolve_path(
        workspace_root,
        &config
            .results_root
            .join("processed")
            .join("harness")
            .join(format!("{}_amc.json", run_name)),
    )
}

fn summary_output_path(workspace_root: &Path, config: &SuiteConfig) -> PathBuf {
    resolve_path(
        workspace_root,
        &config
            .results_root
            .join("processed")
            .join("harness")
            .join(format!("{}_summary.json", config.suite_name)),
    )
}

fn comparison_output_path(workspace_root: &Path, config: &SuiteConfig) -> PathBuf {
    resolve_path(
        workspace_root,
        &config
            .results_root
            .join("processed")
            .join("harness")
            .join(format!("{}_comparison.json", config.suite_name)),
    )
}

async fn load_config(path: &Path) -> Result<SuiteConfig> {
    let bytes = fs::read(path)
        .await
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse config {}", path.display()))
}

#[derive(Clone, Copy)]
enum SuiteCommand {
    Run,
    Analyze,
}

async fn preflight_suite(
    workspace_root: &Path,
    config: &SuiteConfig,
    command: SuiteCommand,
) -> Result<()> {
    validate_suite_config(config)?;

    let replay_manifest_path = resolve_path(workspace_root, &config.replay_manifest);
    ensure_file_exists(&replay_manifest_path, "replay manifest").await?;

    ensure_parent_dir(&summary_output_path(workspace_root, config)).await?;
    ensure_parent_dir(&comparison_output_path(workspace_root, config)).await?;

    if matches!(command, SuiteCommand::Run) {
        ensure_parent_dir(&resolve_path(workspace_root, &config.cert_path)).await?;
    }

    for (index, run) in config.runs.iter().enumerate() {
        let scenario = find_network_scenario(&config.network_scenarios, &run.network_scenario)
            .ok_or_else(|| anyhow!("unknown network scenario {}", run.network_scenario))?;
        let port = config.base_port + index as u16;
        let _server_addr: SocketAddr = format!("{}:{}", config.host, port)
            .parse()
            .with_context(|| format!("invalid host/port {}:{}", config.host, port))?;

        let report_path = report_output_path(workspace_root, config, &run.name);
        let amc_analysis_path = amc_output_path(workspace_root, config, &run.name);

        match command {
            SuiteCommand::Run => {
                validate_network_scenario_for_run(scenario)
                    .with_context(|| format!("network preflight failed for run {}", run.name))?;
                ensure_parent_dir(&report_path).await?;
            }
            SuiteCommand::Analyze => {
                ensure_file_exists(&report_path, &format!("raw report for run {}", run.name))
                    .await?;
            }
        }

        ensure_parent_dir(&amc_analysis_path).await?;
    }

    Ok(())
}

async fn write_summary(path: &Path, summary: &SuiteSummary) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(summary).context("failed to encode harness summary")?;
    fs::write(path, bytes)
        .await
        .with_context(|| format!("failed to write {}", path.display()))
}

async fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    Ok(())
}

async fn ensure_file_exists(path: &Path, label: &str) -> Result<()> {
    let metadata = fs::metadata(path)
        .await
        .with_context(|| format!("missing {} at {}", label, path.display()))?;
    if metadata.is_file() {
        Ok(())
    } else {
        Err(anyhow!("{} at {} is not a file", label, path.display()))
    }
}

fn workspace_root() -> Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .context("failed to resolve workspace root")
}

fn resolve_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn path_relative_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

async fn remove_file_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error)
            .with_context(|| format!("failed to remove stale file {}", path.display())),
    }
}

async fn wait_for_server_certificate(path: &Path, timeout_ms: u64) -> Result<()> {
    let deadline = TokioInstant::now() + TokioDuration::from_millis(timeout_ms.max(50));

    loop {
        if certificate_file_is_ready(path).await? {
            return Ok(());
        }

        if TokioInstant::now() >= deadline {
            return Err(anyhow!(
                "timed out waiting for a valid server certificate at {}",
                path.display()
            ));
        }

        sleep(TokioDuration::from_millis(25)).await;
    }
}

async fn certificate_file_is_ready(path: &Path) -> Result<bool> {
    certificate_is_ready(path).await
}
