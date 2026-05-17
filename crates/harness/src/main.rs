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
    ArtifactProvenance, CoexistenceOutcome, RunInputProvenance, RunOutcome, SkippedRun,
    SuiteInputProvenance, SuiteSummary, analyze_report,
    build_suite_comparison_export, compute_fairness_metrics, load_replay_manifest,
    load_transfer_report, write_amc_analysis, write_comparison_export,
};
use anyhow::{Context, Result, anyhow};
use clap::Parser;
use config::{SuiteConfig, find_network_scenario, validate_suite_config};
use demo_client::{Args as ClientArgs, inspect_replay_input, prepare_replay_input, run_prepared};
use demo_server::{SuiteServer, TransferReport};
use network::{apply_network_scenario, validate_network_scenario_for_run};
use plot::plot_comparison_export;
use sha2::{Digest, Sha256};
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
    let replay_inventory = inspect_replay_input(&replay_manifest_path).await?;
    let replay_dependency_paths = replay_dependency_paths(&replay_manifest_path, &replay_inventory);
    let replay_manifest = load_replay_manifest(&replay_manifest_path).await?;
    let prepared_replay = prepare_replay_input(&replay_manifest_path).await?;
    let suite_input_provenance =
        suite_input_provenance(workspace_root, config_path, &replay_manifest_path).await?;
    let cert_path = resolve_path(workspace_root, &config.cert_path);
    let suite_server_addr: SocketAddr = format!("{}:{}", config.host, config.base_port)
        .parse()
        .with_context(|| format!("invalid host/port {}:{}", config.host, config.base_port))?;
    let suite_server = Arc::new(SuiteServer::bind(suite_server_addr, cert_path.clone()).await?);
    let coexistence_suite_server_addr = coexistence_server_addr(config)?;
    let coexistence_suite_server = if let Some(addr) = coexistence_suite_server_addr {
        Some(Arc::new(SuiteServer::bind(addr, cert_path.clone()).await?))
    } else {
        None
    };
    let mut runs = Vec::with_capacity(config.runs.len());

    for run in &config.runs {
        let network_scenario =
            find_network_scenario(&config.network_scenarios, &run.network_scenario)
                .ok_or_else(|| anyhow!("unknown network scenario {}", run.network_scenario))?;
        let absolute_report_path = report_output_path(workspace_root, config, &run.name);
        let amc_analysis_path = amc_output_path(workspace_root, config, &run.name);
        let coexistence_report_path = coexistence_report_output_path(workspace_root, config, run);
        let coexistence_amc_path = coexistence_amc_output_path(workspace_root, config, run);
        let report_is_reusable =
            report_is_fresh(&absolute_report_path, &replay_dependency_paths).await?
                && if run.coexistence.is_some() {
                    report_is_fresh(&coexistence_report_path, &replay_dependency_paths).await?
                } else {
                    true
                };
        let (transfer_report, coexistence_report) = if report_is_reusable {
            info!(run = %run.name, report = %path_relative_to(workspace_root, &absolute_report_path), "skipping transport because raw report is fresh");
            (
                load_transfer_report(&absolute_report_path).await?,
                if run.coexistence.is_some() {
                    Some(load_transfer_report(&coexistence_report_path).await?)
                } else {
                    None
                },
            )
        } else {
            let _network_guard = apply_network_scenario(network_scenario)?;
            info!(run = %run.name, mode = ?run.mode, pace = ?run.pace, scenario = %network_scenario.name, server = %suite_server_addr, "starting harness run");

            let server = suite_server.clone();
            let report_out = absolute_report_path.clone();
            let server_task: JoinHandle<Result<TransferReport>> =
                tokio::spawn(async move { server.run_transfer(&report_out).await });

            if let Some(coexistence) = run.coexistence.as_ref() {
                let coexistence_server = coexistence_suite_server
                    .as_ref()
                    .cloned()
                    .context("coexistence run requested without coexistence server")?;
                let coexistence_server_addr = coexistence_suite_server_addr
                    .context("coexistence run requested without coexistence server address")?;
                let coexistence_report_out = coexistence_report_path.clone();
                let coexistence_server_for_task = coexistence_server.clone();
                let coexistence_server_task: JoinHandle<Result<TransferReport>> =
                    tokio::spawn(async move {
                        coexistence_server_for_task
                            .run_transfer(&coexistence_report_out)
                            .await
                    });

                let (client_summary, coexistence_summary) = tokio::try_join!(
                    run_prepared(
                        client_args_from_run(
                            suite_server_addr,
                            &cert_path,
                            &replay_manifest_path,
                            run,
                        ),
                        &prepared_replay,
                        suite_server.cert_der(),
                    ),
                    run_prepared(
                        client_args_from_coexistence(
                            coexistence_server_addr,
                            &cert_path,
                            &replay_manifest_path,
                            coexistence,
                        ),
                        &prepared_replay,
                        coexistence_server.cert_der(),
                    ),
                )
                .with_context(|| format!("client coexistence run {} failed", run.name))?;

                let transfer_report = server_task
                    .await
                    .context("foreground server task join failed")?
                    .with_context(|| format!("server run {} failed", run.name))?;
                let coexistence_report = coexistence_server_task
                    .await
                    .context("coexistence server task join failed")?
                    .with_context(|| format!("coexistence server run {} failed", run.name))?;

                ensure_report_path_match(
                    &run.name,
                    "foreground",
                    &client_summary.report_path,
                    &transfer_report.summary.report_path,
                )?;
                ensure_report_path_match(
                    &run.name,
                    "coexistence",
                    &coexistence_summary.report_path,
                    &coexistence_report.summary.report_path,
                )?;

                (transfer_report, Some(coexistence_report))
            } else {
                let client_summary = run_prepared(
                    client_args_from_run(suite_server_addr, &cert_path, &replay_manifest_path, run),
                    &prepared_replay,
                    suite_server.cert_der(),
                )
                .await
                .with_context(|| format!("client run {} failed", run.name))?;

                let transfer_report = server_task
                    .await
                    .context("server task join failed")?
                    .with_context(|| format!("server run {} failed", run.name))?;

                ensure_report_path_match(
                    &run.name,
                    "foreground",
                    &client_summary.report_path,
                    &transfer_report.summary.report_path,
                )?;

                (transfer_report, None)
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
        let mut amc_analysis = analyze_report(
            run,
            network_scenario,
            &config.semantic_profile,
            &replay_manifest,
            &transfer_report,
        );
        amc_analysis.input_provenance = Some(
            run_input_provenance(workspace_root, &suite_input_provenance, &absolute_report_path)
                .await?,
        );
        write_amc_analysis(&amc_analysis_path, &amc_analysis).await?;

        let coexistence = if let (Some(coexistence_config), Some(coexistence_report)) =
            (run.coexistence.as_ref(), coexistence_report)
        {
            if coexistence_report.summary.baseline_controller != coexistence_config.controller {
                return Err(anyhow!(
                    "coexistence controller mismatch for {}: run config {:?} vs raw report {:?}",
                    run.name,
                    coexistence_config.controller,
                    coexistence_report.summary.baseline_controller
                ));
            }
            let mut coexistence_analysis = analyze_report(
                run,
                network_scenario,
                &config.semantic_profile,
                &replay_manifest,
                &coexistence_report,
            );
            coexistence_analysis.input_provenance = Some(
                run_input_provenance(
                    workspace_root,
                    &suite_input_provenance,
                    &coexistence_report_path,
                )
                .await?,
            );
            write_amc_analysis(&coexistence_amc_path, &coexistence_analysis).await?;

            Some(CoexistenceOutcome {
                controller: coexistence_config.controller,
                mode: coexistence_config.mode,
                pace: coexistence_config.pace,
                report_path: path_relative_to(workspace_root, &coexistence_report_path),
                amc_analysis_path: path_relative_to(workspace_root, &coexistence_amc_path),
                amc_aggregate: coexistence_analysis.aggregate.clone(),
                summary: coexistence_report.summary.clone(),
                fairness: compute_fairness_metrics(
                    &amc_analysis.aggregate,
                    &coexistence_analysis.aggregate,
                ),
            })
        } else {
            None
        };

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
            coexistence,
        });
    }

    suite_server.shutdown().await;
    if let Some(server) = coexistence_suite_server {
        server.shutdown().await;
    }

    write_suite_summary(
        workspace_root,
        config,
        runs,
        Vec::new(),
        Some(suite_input_provenance),
    )
    .await
}

async fn analyze_existing_suite(
    workspace_root: &Path,
    config_path: &Path,
    config: &SuiteConfig,
) -> Result<PathBuf> {
    let replay_manifest_path = resolve_path(workspace_root, &config.replay_manifest);
    let replay_inventory = inspect_replay_input(&replay_manifest_path).await?;
    let _replay_dependency_paths = replay_dependency_paths(&replay_manifest_path, &replay_inventory);
    let replay_manifest = load_replay_manifest(&replay_manifest_path).await?;
    let suite_input_provenance =
        suite_input_provenance(workspace_root, config_path, &replay_manifest_path).await?;
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
        let coexistence_report_path = coexistence_report_output_path(workspace_root, config, run);
        let coexistence_amc_path = coexistence_amc_output_path(workspace_root, config, run);
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
        let coexistence_report = if run.coexistence.is_some() {
            match load_transfer_report(&coexistence_report_path).await {
                Ok(report) => Some(report),
                Err(error) => {
                    warn!(run = %run.name, report = %path_relative_to(workspace_root, &coexistence_report_path), error = %error, "skipping run without coexistence raw report during analyze-suite");
                    skipped_runs.push(SkippedRun {
                        name: run.name.clone(),
                        controller: run.controller,
                        mode: run.mode,
                        pace: run.pace,
                        network_scenario: run.network_scenario.clone(),
                        expected_report_path: path_relative_to(
                            workspace_root,
                            &coexistence_report_path,
                        ),
                        reason: "missing coexistence raw report".to_string(),
                    });
                    continue;
                }
            }
        } else {
            None
        };
        if transfer_report.summary.baseline_controller != run.controller {
            return Err(anyhow!(
                "controller mismatch for {}: run config {:?} vs raw report {:?}",
                run.name,
                run.controller,
                transfer_report.summary.baseline_controller
            ));
        }
        let mut amc_analysis = analyze_report(
            run,
            network_scenario,
            &config.semantic_profile,
            &replay_manifest,
            &transfer_report,
        );
        amc_analysis.input_provenance = Some(
            run_input_provenance(workspace_root, &suite_input_provenance, &absolute_report_path)
                .await?,
        );
        write_amc_analysis(&amc_analysis_path, &amc_analysis).await?;

        let coexistence = if let (Some(coexistence_config), Some(coexistence_report)) =
            (run.coexistence.as_ref(), coexistence_report)
        {
            if coexistence_report.summary.baseline_controller != coexistence_config.controller {
                return Err(anyhow!(
                    "coexistence controller mismatch for {}: run config {:?} vs raw report {:?}",
                    run.name,
                    coexistence_config.controller,
                    coexistence_report.summary.baseline_controller
                ));
            }
            let mut coexistence_analysis = analyze_report(
                run,
                network_scenario,
                &config.semantic_profile,
                &replay_manifest,
                &coexistence_report,
            );
            coexistence_analysis.input_provenance = Some(
                run_input_provenance(
                    workspace_root,
                    &suite_input_provenance,
                    &coexistence_report_path,
                )
                .await?,
            );
            write_amc_analysis(&coexistence_amc_path, &coexistence_analysis).await?;

            Some(CoexistenceOutcome {
                controller: coexistence_config.controller,
                mode: coexistence_config.mode,
                pace: coexistence_config.pace,
                report_path: path_relative_to(workspace_root, &coexistence_report_path),
                amc_analysis_path: path_relative_to(workspace_root, &coexistence_amc_path),
                amc_aggregate: coexistence_analysis.aggregate.clone(),
                summary: coexistence_report.summary.clone(),
                fairness: compute_fairness_metrics(
                    &amc_analysis.aggregate,
                    &coexistence_analysis.aggregate,
                ),
            })
        } else {
            None
        };

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
            coexistence,
        });
    }

    if runs.is_empty() {
        return Err(anyhow!(
            "analyze-suite did not find any raw reports for {}; run `cargo run -p harness -- run-suite --config {}` first",
            config.suite_name,
            path_relative_to(workspace_root, config_path)
        ));
    }

    write_suite_summary(
        workspace_root,
        config,
        runs,
        skipped_runs,
        Some(suite_input_provenance),
    )
    .await
}

async fn write_suite_summary(
    workspace_root: &Path,
    config: &SuiteConfig,
    runs: Vec<RunOutcome>,
    skipped_runs: Vec<SkippedRun>,
    input_provenance: Option<SuiteInputProvenance>,
) -> Result<PathBuf> {
    let summary = SuiteSummary {
        suite_name: config.suite_name.clone(),
        replay_manifest: path_relative_to(
            workspace_root,
            &resolve_path(workspace_root, &config.replay_manifest),
        ),
        input_provenance: input_provenance.clone(),
        network_scenarios: config.network_scenarios.clone(),
        runs,
        skipped_runs,
    };
    let summary_path = summary_output_path(workspace_root, config);
    write_summary(&summary_path, &summary).await?;
    let comparison_path = comparison_output_path(workspace_root, config);
    let mut comparison_export = build_suite_comparison_export(
        &summary.suite_name,
        &summary.replay_manifest,
        &summary.network_scenarios,
        &config.runs,
        &summary.runs,
        &summary.skipped_runs,
    );
    comparison_export.input_provenance = input_provenance;
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

fn coexistence_report_output_path(
    workspace_root: &Path,
    config: &SuiteConfig,
    run: &config::RunConfig,
) -> PathBuf {
    resolve_path(
        workspace_root,
        &config
            .results_root
            .join("raw")
            .join("harness")
            .join(format!("{}_coexistence_report.json", run.name)),
    )
}

fn coexistence_amc_output_path(
    workspace_root: &Path,
    config: &SuiteConfig,
    run: &config::RunConfig,
) -> PathBuf {
    resolve_path(
        workspace_root,
        &config
            .results_root
            .join("processed")
            .join("harness")
            .join(format!("{}_coexistence_amc.json", run.name)),
    )
}

fn client_args_from_run(
    server: SocketAddr,
    cert_path: &Path,
    replay_manifest_path: &Path,
    run: &config::RunConfig,
) -> ClientArgs {
    ClientArgs {
        bind: "0.0.0.0:0".parse().unwrap(),
        server,
        server_name: "localhost".to_string(),
        cert: cert_path.to_path_buf(),
        replay_manifest: replay_manifest_path.to_path_buf(),
        pace: run.pace,
        controller: run.controller,
        mode: run.mode,
        vod_deadline_slack_ms: run.vod_deadline_slack_ms.unwrap_or(30_000),
    }
}

fn client_args_from_coexistence(
    server: SocketAddr,
    cert_path: &Path,
    replay_manifest_path: &Path,
    coexistence: &config::CoexistenceConfig,
) -> ClientArgs {
    ClientArgs {
        bind: "0.0.0.0:0".parse().unwrap(),
        server,
        server_name: "localhost".to_string(),
        cert: cert_path.to_path_buf(),
        replay_manifest: replay_manifest_path.to_path_buf(),
        pace: coexistence.pace,
        controller: coexistence.controller,
        mode: coexistence.mode,
        vod_deadline_slack_ms: coexistence.vod_deadline_slack_ms.unwrap_or(30_000),
    }
}

fn ensure_report_path_match(
    run_name: &str,
    label: &str,
    client_report_path: &str,
    server_report_path: &str,
) -> Result<()> {
    if client_report_path != server_report_path {
        Err(anyhow!(
            "{} report path mismatch for {}: {} vs {}",
            label,
            run_name,
            client_report_path,
            server_report_path
        ))
    } else {
        Ok(())
    }
}

fn coexistence_server_addr(config: &SuiteConfig) -> Result<Option<SocketAddr>> {
    if !config.runs.iter().any(|run| run.coexistence.is_some()) {
        return Ok(None);
    }

    let coexistence_port = config
        .base_port
        .checked_add(1000)
        .context("coexistence server port overflowed u16 range")?;
    let addr = format!("{}:{}", config.host, coexistence_port)
        .parse()
        .with_context(|| {
            format!(
                "invalid coexistence host/port {}:{}",
                config.host, coexistence_port
            )
        })?;
    Ok(Some(addr))
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
    let replay_inventory = inspect_replay_input(&replay_manifest_path)
        .await
        .with_context(|| {
            format!(
                "replay input preflight failed for {}",
                replay_manifest_path.display()
            )
        })?;
    info!(
        asset = %replay_inventory.asset_name,
        schema_version = replay_inventory.schema_version,
        segment_count = replay_inventory.segment_paths.len(),
        total_payload_bytes = replay_inventory.total_payload_bytes,
        "replay input preflight passed"
    );

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
        let _ = coexistence_server_addr(config)?;
    }

    for (index, run) in config.runs.iter().enumerate() {
        let scenario = find_network_scenario(&config.network_scenarios, &run.network_scenario)
            .ok_or_else(|| anyhow!("unknown network scenario {}", run.network_scenario))?;
        let port = config.base_port + index as u16;

        let report_path = report_output_path(workspace_root, config, &run.name);
        let amc_analysis_path = amc_output_path(workspace_root, config, &run.name);
        let coexistence_report_path = coexistence_report_output_path(workspace_root, config, run);
        let coexistence_amc_path = coexistence_amc_output_path(workspace_root, config, run);
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
                if run.coexistence.is_some() {
                    ensure_parent_dir(&coexistence_report_path).await?;
                }
            }
            SuiteCommand::Analyze => {
                let _ = report_path;
            }
        }

        ensure_parent_dir(&amc_analysis_path).await?;
        if run.coexistence.is_some() {
            ensure_distinct_output_path(
                &mut output_paths,
                &format!("coexistence raw report for run {}", run.name),
                &coexistence_report_path,
            )?;
            ensure_distinct_output_path(
                &mut output_paths,
                &format!("coexistence AMC analysis for run {}", run.name),
                &coexistence_amc_path,
            )?;
            ensure_parent_dir(&coexistence_amc_path).await?;
        }
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

fn replay_dependency_paths<'a>(
    replay_manifest_path: &'a Path,
    replay_inventory: &'a demo_client::ReplayAssetInventory,
) -> Vec<&'a Path> {
    let mut dependency_paths = Vec::with_capacity(2 + replay_inventory.segment_paths.len());
    dependency_paths.push(replay_manifest_path);
    dependency_paths.push(replay_inventory.init_segment_path.as_path());
    dependency_paths.extend(
        replay_inventory
            .segment_paths
            .iter()
            .map(std::path::PathBuf::as_path),
    );
    dependency_paths
}

async fn suite_input_provenance(
    workspace_root: &Path,
    config_path: &Path,
    replay_manifest_path: &Path,
) -> Result<SuiteInputProvenance> {
    Ok(SuiteInputProvenance {
        suite_config: artifact_provenance(workspace_root, config_path).await?,
        replay_manifest: artifact_provenance(workspace_root, replay_manifest_path).await?,
    })
}

async fn run_input_provenance(
    workspace_root: &Path,
    suite_input_provenance: &SuiteInputProvenance,
    raw_report_path: &Path,
) -> Result<RunInputProvenance> {
    Ok(RunInputProvenance {
        suite_config: suite_input_provenance.suite_config.clone(),
        replay_manifest: suite_input_provenance.replay_manifest.clone(),
        raw_report: artifact_provenance(workspace_root, raw_report_path).await?,
    })
}

async fn artifact_provenance(workspace_root: &Path, path: &Path) -> Result<ArtifactProvenance> {
    let bytes = fs::read(path)
        .await
        .with_context(|| format!("failed to read {} for provenance", path.display()))?;
    let digest = Sha256::digest(&bytes);
    Ok(ArtifactProvenance {
        path: path_relative_to(workspace_root, path),
        sha256: hex_digest(&digest),
    })
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use demo_client::{Args as ClientArgs, BaselineController, Pace, ReplayMode, prepare_replay_input, run_prepared};
    use std::{env, fs as stdfs, sync::Arc, time::{SystemTime, UNIX_EPOCH}};
    use tokio::{fs, task::JoinHandle};

    fn unique_temp_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        env::temp_dir().join(format!("quinn-amc-{label}-{}-{unique}", std::process::id()))
    }

    async fn write_replay_fixture(root: &Path) -> Result<PathBuf> {
        let manifests_dir = root.join("data/processed/manifests");
        let segments_dir = root.join("data/processed/segments/test_asset");
        fs::create_dir_all(&manifests_dir).await?;
        fs::create_dir_all(&segments_dir).await?;

        fs::write(segments_dir.join("test_asset_init.mp4"), b"init-bytes").await?;
        fs::write(segments_dir.join("test_asset_chunk_00001.m4s"), b"segment-bytes").await?;

        let manifest_path = manifests_dir.join("test_asset_replay.json");
        let manifest = serde_json::json!({
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
                    "size_bytes": 13,
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
    async fn client_server_round_trip_preserves_summary_controller() -> Result<()> {
        let root = unique_temp_dir("summary-round-trip");
        fs::create_dir_all(&root).await?;
        let manifest_path = write_replay_fixture(&root).await?;
        let replay_input = prepare_replay_input(&manifest_path).await?;

        let cert_path = root.join("results/test-cert.der");
        let report_path = root.join("results/raw/harness/test_report.json");
        let server = Arc::new(
            SuiteServer::bind("127.0.0.1:0".parse::<SocketAddr>()?, cert_path.clone()).await?,
        );
        let server_addr = server.local_addr()?;
        let server_task: JoinHandle<Result<TransferReport>> = {
            let server = Arc::clone(&server);
            let report_path = report_path.clone();
            tokio::spawn(async move { server.run_transfer(&report_path).await })
        };

        let client_summary = run_prepared(
            ClientArgs {
                bind: "127.0.0.1:0".parse()?,
                server: server_addr,
                server_name: "localhost".to_string(),
                cert: cert_path,
                replay_manifest: manifest_path,
                pace: Pace::Immediate,
                mode: ReplayMode::Live,
                vod_deadline_slack_ms: 30_000,
                controller: BaselineController::Bbr,
            },
            &replay_input,
            server.cert_der(),
        )
        .await?;

        let report = server_task.await??;
        server.shutdown().await;

        assert_eq!(client_summary.baseline_controller, BaselineController::Bbr);
        assert_eq!(report.summary.baseline_controller, BaselineController::Bbr);
        assert_eq!(client_summary.report_path, report.summary.report_path);

        stdfs::remove_dir_all(&root)?;
        Ok(())
    }
}
