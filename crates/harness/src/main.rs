mod analysis;
mod config;
mod network;
mod plot;
mod tui_demo;

use std::{
    collections::HashMap,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};

use analysis::{
    RunOutcome, SkippedRun, SuiteSummary, analyze_report, build_suite_comparison_export,
    load_replay_manifest, load_transfer_report, write_amc_analysis, write_comparison_export,
};
use anyhow::{Context, Result, anyhow};
use clap::Parser;
use config::{SuiteConfig, find_network_scenario, validate_suite_config};
use demo_client::{Args as ClientArgs, prepare_replay_input, run_prepared};
use demo_server::{SuiteServer, TransferReport};
use network::{apply_network_scenario, validate_network_scenario_for_run};
use plot::plot_comparison_export;
use tokio::{fs, task::JoinHandle};
use tracing::{info, warn};

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
    PlotSuite {
        #[arg(long)]
        comparison: PathBuf,
        #[arg(long, default_value = "results/figures/harness")]
        output_dir: PathBuf,
    },
    LiveDemo {
        #[arg(long)]
        report: PathBuf,
        #[arg(long, default_value_t = 1.0)]
        speed: f64,
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
            preflight_suite(&workspace_root, &config_path, &config, SuiteCommand::Run).await?;
            let summary_path = run_local_suite(&workspace_root, &config_path, &config).await?;
            info!(summary_path = %path_relative_to(&workspace_root, &summary_path), "harness suite completed");
        }
        Command::AnalyzeSuite { config } => {
            let config_path = resolve_path(&workspace_root, &config);
            let config = load_config(&config_path).await?;
            preflight_suite(
                &workspace_root,
                &config_path,
                &config,
                SuiteCommand::Analyze,
            )
            .await?;
            let summary_path =
                analyze_existing_suite(&workspace_root, &config_path, &config).await?;
            info!(summary_path = %path_relative_to(&workspace_root, &summary_path), "harness analysis completed");
        }
        Command::PlotSuite {
            comparison,
            output_dir,
        } => {
            let comparison_path = resolve_path(&workspace_root, &comparison);
            let output_dir = resolve_path(&workspace_root, &output_dir);
            let outputs = plot_comparison_export(&comparison_path, &output_dir).await?;
            for output in outputs {
                info!(figure = %path_relative_to(&workspace_root, &output), "harness figure written");
            }
        }
        Command::LiveDemo { report, speed } => {
            let report_path = resolve_path(&workspace_root, &report);
            let report = load_transfer_report(&report_path).await?;
            tui_demo::run_report_replay(&report, speed)?;
        }
    }

    Ok(())
}

async fn run_local_suite(
    workspace_root: &Path,
    config_path: &Path,
    config: &SuiteConfig,
) -> Result<PathBuf> {
    let replay_manifest_path = resolve_path(workspace_root, &config.replay_manifest);
    let replay_manifest = load_replay_manifest(&replay_manifest_path).await?;
    let prepared_replay = prepare_replay_input(&replay_manifest_path).await?;
    let cert_path = resolve_path(workspace_root, &config.cert_path);
    let suite_server_addr: SocketAddr = format!("{}:{}", config.host, config.base_port)
        .parse()
        .with_context(|| format!("invalid host/port {}:{}", config.host, config.base_port))?;
    let suite_server = Arc::new(SuiteServer::bind(suite_server_addr, cert_path.clone()).await?);
    let mut runs = Vec::with_capacity(config.runs.len());

    for run in &config.runs {
        let network_scenario =
            find_network_scenario(&config.network_scenarios, &run.network_scenario)
                .ok_or_else(|| anyhow!("unknown network scenario {}", run.network_scenario))?;
        let absolute_report_path = report_output_path(workspace_root, config, &run.name);
        let amc_analysis_path = amc_output_path(workspace_root, config, &run.name);
        let transfer_report = if report_is_fresh(
            &absolute_report_path,
            &[config_path, &replay_manifest_path],
        )
        .await?
        {
            info!(run = %run.name, report = %path_relative_to(workspace_root, &absolute_report_path), "skipping transport because raw report is fresh");
            load_transfer_report(&absolute_report_path).await?
        } else {
            let _network_guard = apply_network_scenario(network_scenario)?;
            info!(run = %run.name, mode = ?run.mode, pace = ?run.pace, scenario = %network_scenario.name, server = %suite_server_addr, "starting harness run");

            let server = suite_server.clone();
            let report_out = absolute_report_path.clone();
            let server_task: JoinHandle<Result<TransferReport>> =
                tokio::spawn(async move { server.run_transfer(&report_out).await });

            let client_summary = run_prepared(
                ClientArgs {
                    bind: "0.0.0.0:0".parse().unwrap(),
                    server: suite_server_addr,
                    server_name: "localhost".to_string(),
                    cert: cert_path.clone(),
                    replay_manifest: replay_manifest_path.clone(),
                    pace: run.pace,
                    controller: run.controller,
                    mode: run.mode,
                    vod_deadline_slack_ms: run.vod_deadline_slack_ms.unwrap_or(30_000),
                },
                &prepared_replay,
                suite_server.cert_der(),
            )
            .await
            .with_context(|| format!("client run {} failed", run.name))?;

            let transfer_report = server_task
                .await
                .context("server task join failed")?
                .with_context(|| format!("server run {} failed", run.name))?;

            if client_summary.report_path != transfer_report.summary.report_path {
                return Err(anyhow!(
                    "client/server report path mismatch for {}: {} vs {}",
                    run.name,
                    client_summary.report_path,
                    transfer_report.summary.report_path
                ));
            }

            transfer_report
        };

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
            server: suite_server_addr.to_string(),
            report_path: path_relative_to(workspace_root, &absolute_report_path),
            network_scenario: network_scenario.clone(),
            amc_analysis_path: path_relative_to(workspace_root, &amc_analysis_path),
            amc_aggregate: amc_analysis.aggregate.clone(),
            summary: transfer_report.summary.clone(),
        });
    }

    suite_server.shutdown().await;

    write_suite_summary(workspace_root, config, runs, Vec::new()).await
}

async fn analyze_existing_suite(
    workspace_root: &Path,
    config_path: &Path,
    config: &SuiteConfig,
) -> Result<PathBuf> {
    let replay_manifest_path = resolve_path(workspace_root, &config.replay_manifest);
    let replay_manifest = load_replay_manifest(&replay_manifest_path).await?;
    let mut runs = Vec::with_capacity(config.runs.len());
    let mut skipped_runs = Vec::new();

    for (index, run) in config.runs.iter().enumerate() {
        let network_scenario =
            find_network_scenario(&config.network_scenarios, &run.network_scenario)
                .ok_or_else(|| anyhow!("unknown network scenario {}", run.network_scenario))?;
        let port = config.base_port + index as u16;
        let server_addr = format!("{}:{}", config.host, port);
        let absolute_report_path = report_output_path(workspace_root, config, &run.name);
        let amc_analysis_path = amc_output_path(workspace_root, config, &run.name);
        let transfer_report = match load_transfer_report(&absolute_report_path).await {
            Ok(report) => report,
            Err(error) => {
                warn!(run = %run.name, report = %path_relative_to(workspace_root, &absolute_report_path), error = %error, "skipping run without raw report during analyze-suite");
                skipped_runs.push(SkippedRun {
                    name: run.name.clone(),
                    controller: run.controller,
                    mode: run.mode,
                    pace: run.pace,
                    network_scenario: run.network_scenario.clone(),
                    expected_report_path: path_relative_to(workspace_root, &absolute_report_path),
                    reason: "missing raw report".to_string(),
                });
                continue;
            }
        };
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

    if runs.is_empty() {
        return Err(anyhow!(
            "analyze-suite did not find any raw reports for {}; run `cargo run -p harness -- run-suite --config {}` first",
            config.suite_name,
            path_relative_to(workspace_root, config_path)
        ));
    }

    write_suite_summary(workspace_root, config, runs, skipped_runs).await
}

async fn write_suite_summary(
    workspace_root: &Path,
    config: &SuiteConfig,
    runs: Vec<RunOutcome>,
    skipped_runs: Vec<SkippedRun>,
) -> Result<PathBuf> {
    let summary = SuiteSummary {
        suite_name: config.suite_name.clone(),
        replay_manifest: path_relative_to(
            workspace_root,
            &resolve_path(workspace_root, &config.replay_manifest),
        ),
        network_scenarios: config.network_scenarios.clone(),
        runs,
        skipped_runs,
    };
    let summary_path = summary_output_path(workspace_root, config);
    write_summary(&summary_path, &summary).await?;
    let comparison_path = comparison_output_path(workspace_root, config);
    let comparison_export = build_suite_comparison_export(
        &summary.suite_name,
        &summary.replay_manifest,
        &summary.network_scenarios,
        &config.runs,
        &summary.runs,
        &summary.skipped_runs,
    );
    write_comparison_export(&comparison_path, &comparison_export).await?;
    Ok(summary_path)
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
    _config_path: &Path,
    config: &SuiteConfig,
    command: SuiteCommand,
) -> Result<()> {
    validate_suite_config(config)?;

    let replay_manifest_path = resolve_path(workspace_root, &config.replay_manifest);
    ensure_file_exists(&replay_manifest_path, "replay manifest").await?;

    let summary_path = summary_output_path(workspace_root, config);
    let comparison_path = comparison_output_path(workspace_root, config);
    let mut output_paths = HashMap::new();
    ensure_distinct_output_path(&mut output_paths, "suite summary", &summary_path)?;
    ensure_distinct_output_path(
        &mut output_paths,
        "suite comparison export",
        &comparison_path,
    )?;

    ensure_parent_dir(&summary_path).await?;
    ensure_parent_dir(&comparison_path).await?;

    if matches!(command, SuiteCommand::Run) {
        ensure_parent_dir(&resolve_path(workspace_root, &config.cert_path)).await?;
    }

    for (index, run) in config.runs.iter().enumerate() {
        let scenario = find_network_scenario(&config.network_scenarios, &run.network_scenario)
            .ok_or_else(|| anyhow!("unknown network scenario {}", run.network_scenario))?;
        let port = config.base_port + index as u16;

        let report_path = report_output_path(workspace_root, config, &run.name);
        let amc_analysis_path = amc_output_path(workspace_root, config, &run.name);
        ensure_distinct_output_path(
            &mut output_paths,
            &format!("raw report for run {}", run.name),
            &report_path,
        )?;
        ensure_distinct_output_path(
            &mut output_paths,
            &format!("AMC analysis for run {}", run.name),
            &amc_analysis_path,
        )?;

        match command {
            SuiteCommand::Run => {
                let _server_addr: SocketAddr = format!("{}:{}", config.host, port)
                    .parse()
                    .with_context(|| format!("invalid host/port {}:{}", config.host, port))?;
                validate_network_scenario_for_run(scenario)
                    .with_context(|| format!("network preflight failed for run {}", run.name))?;
                ensure_parent_dir(&report_path).await?;
            }
            SuiteCommand::Analyze => {
                let _ = report_path;
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

fn ensure_distinct_output_path(
    output_paths: &mut HashMap<PathBuf, String>,
    label: &str,
    path: &Path,
) -> Result<()> {
    match output_paths.insert(path.to_path_buf(), label.to_string()) {
        Some(existing_label) => Err(anyhow!(
            "output path collision between {} and {} at {}",
            existing_label,
            label,
            path.display()
        )),
        None => Ok(()),
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

async fn report_is_fresh(report_path: &Path, dependency_paths: &[&Path]) -> Result<bool> {
    let report_metadata = match fs::metadata(report_path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to inspect {}", report_path.display()));
        }
    };

    if !report_metadata.is_file() {
        return Ok(false);
    }

    let report_modified = report_metadata
        .modified()
        .with_context(|| format!("failed to read mtime for {}", report_path.display()))?;

    for dependency_path in dependency_paths {
        let dependency_modified = fs::metadata(dependency_path)
            .await
            .with_context(|| format!("failed to inspect {}", dependency_path.display()))?
            .modified()
            .with_context(|| format!("failed to read mtime for {}", dependency_path.display()))?;
        if dependency_modified > report_modified {
            return Ok(false);
        }
    }

    Ok(true)
}
