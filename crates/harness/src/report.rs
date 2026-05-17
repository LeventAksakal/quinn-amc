use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow, ensure};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tokio::fs;

use crate::analysis::{ArtifactProvenance, SuiteComparisonExport, SuiteInputProvenance};

const MATRIX_SUITE_NAME: &str = "vps_fixed_preset_controller_matrix";
const FAIRNESS_SUITE_NAME: &str = "vps_host_live_coexistence_bbr_guardrail";

const MATRIX_OVERVIEW_METRICS: &[&str] = &[
    "useful_media_ratio",
    "deadline_miss_rate",
    "throughput_mbps",
    "average_delivery_latency_ms",
    "average_jitter_ms",
    "average_age_of_information_ms",
    "vod_startup_delay_ms",
    "vod_rebuffer_ratio",
];

const MATRIX_LIVE_METRICS: &[&str] = &[
    "useful_media_ratio",
    "deadline_miss_rate",
    "throughput_mbps",
    "average_delivery_latency_ms",
    "average_jitter_ms",
    "average_age_of_information_ms",
];

const MATRIX_VOD_METRICS: &[&str] = &[
    "useful_media_ratio",
    "deadline_miss_rate",
    "throughput_mbps",
    "average_delivery_latency_ms",
    "average_jitter_ms",
    "vod_startup_delay_ms",
    "vod_rebuffer_ratio",
];

const FAIRNESS_OVERVIEW_METRICS: &[&str] = &[
    "useful_media_ratio",
    "deadline_miss_rate",
    "throughput_mbps",
    "average_delivery_latency_ms",
    "average_jitter_ms",
    "average_age_of_information_ms",
    "fairness_foreground_throughput_share",
    "fairness_throughput_ratio",
    "fairness_jain_index",
];

const FAIRNESS_LIVE_METRICS: &[&str] = &[
    "useful_media_ratio",
    "deadline_miss_rate",
    "throughput_mbps",
    "average_delivery_latency_ms",
    "average_jitter_ms",
    "average_age_of_information_ms",
    "fairness_foreground_throughput_share",
    "fairness_throughput_ratio",
    "fairness_jain_index",
];

pub struct ReportPackageResult {
    pub output_dir: PathBuf,
    pub report_path: PathBuf,
    pub manifest_path: PathBuf,
    pub reproducibility_path: PathBuf,
    pub figure_count: usize,
}

struct PackagedSuiteInput {
    suite_name: &'static str,
    comparison_path: PathBuf,
    summary_path: PathBuf,
    comparison_export: SuiteComparisonExport,
}

#[derive(Clone)]
struct ExpectedFigure {
    suite_name: &'static str,
    file_name: String,
    category: &'static str,
}

#[derive(Serialize)]
struct ReportPackageManifest {
    package_name: &'static str,
    generated_at_epoch_s: u64,
    canonical_report_source: String,
    packaged_report_path: String,
    reproducibility_note_path: String,
    figure_count: usize,
    evidence_artifacts: Vec<PackagedArtifact>,
    figures: Vec<PackagedFigure>,
    suites: Vec<PackagedSuite>,
}

#[derive(Serialize)]
struct PackagedArtifact {
    role: String,
    source_path: String,
    packaged_path: String,
    sha256: String,
}

#[derive(Serialize)]
struct PackagedFigure {
    suite_name: String,
    category: String,
    source_path: String,
    packaged_path: String,
    sha256: String,
}

#[derive(Serialize)]
struct PackagedSuite {
    suite_name: String,
    comparison_path: String,
    summary_path: String,
    complete_groups: usize,
    total_groups: usize,
    row_count: usize,
    input_provenance: Option<SuiteInputProvenance>,
}

pub async fn package_report(
    workspace_root: &Path,
    report_source_path: &Path,
    matrix_comparison_path: &Path,
    fairness_comparison_path: &Path,
    figure_dir: &Path,
    output_dir: &Path,
) -> Result<ReportPackageResult> {
    ensure_file_exists(report_source_path, "canonical report source").await?;
    ensure_dir_exists(figure_dir, "figure directory").await?;

    let suites = vec![
        load_packaged_suite(matrix_comparison_path, MATRIX_SUITE_NAME, 10).await?,
        load_packaged_suite(fairness_comparison_path, FAIRNESS_SUITE_NAME, 2).await?,
    ];

    let expected_figures = expected_figure_inventory();
    validate_expected_figures(figure_dir, &expected_figures).await?;

    fs::create_dir_all(output_dir)
        .await
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    let artifacts_dir = output_dir.join("artifacts");
    let figures_dir = output_dir.join("figures");
    fs::create_dir_all(&artifacts_dir)
        .await
        .with_context(|| format!("failed to create {}", artifacts_dir.display()))?;
    fs::create_dir_all(&figures_dir)
        .await
        .with_context(|| format!("failed to create {}", figures_dir.display()))?;

    let mut packaged_artifacts = Vec::new();
    for suite in &suites {
        packaged_artifacts.push(
            copy_artifact(
                workspace_root,
                output_dir,
                &suite.comparison_path,
                &artifacts_dir.join(file_name(&suite.comparison_path)?),
                format!("{} comparison export", suite.suite_name),
            )
            .await?,
        );
        packaged_artifacts.push(
            copy_artifact(
                workspace_root,
                output_dir,
                &suite.summary_path,
                &artifacts_dir.join(file_name(&suite.summary_path)?),
                format!("{} summary", suite.suite_name),
            )
            .await?,
        );
    }

    let mut packaged_figures = Vec::new();
    for figure in &expected_figures {
        let source_path = figure_dir.join(&figure.file_name);
        let packaged_path = figures_dir.join(&figure.file_name);
        copy_file(&source_path, &packaged_path).await?;
        let provenance = artifact_provenance(workspace_root, &source_path).await?;
        packaged_figures.push(PackagedFigure {
            suite_name: figure.suite_name.to_string(),
            category: figure.category.to_string(),
            source_path: provenance.path,
            packaged_path: path_relative_to(output_dir, &packaged_path),
            sha256: provenance.sha256,
        });
    }

    let report_source = fs::read_to_string(report_source_path)
        .await
        .with_context(|| format!("failed to read {}", report_source_path.display()))?;
    let packaged_report = rewrite_report_links(&report_source);
    let report_path = output_dir.join("report.md");
    fs::write(&report_path, packaged_report)
        .await
        .with_context(|| format!("failed to write {}", report_path.display()))?;

    let reproducibility_path = output_dir.join("reproducibility.md");
    let reproducibility_note = build_reproducibility_note(&suites, expected_figures.len());
    fs::write(&reproducibility_path, reproducibility_note)
        .await
        .with_context(|| format!("failed to write {}", reproducibility_path.display()))?;

    let manifest_path = output_dir.join("manifest.json");
    let manifest = ReportPackageManifest {
        package_name: "phase-7-report-package",
        generated_at_epoch_s: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock before unix epoch")?
            .as_secs(),
        canonical_report_source: path_relative_to(workspace_root, report_source_path),
        packaged_report_path: path_relative_to(output_dir, &report_path),
        reproducibility_note_path: path_relative_to(output_dir, &reproducibility_path),
        figure_count: expected_figures.len(),
        evidence_artifacts: packaged_artifacts,
        figures: packaged_figures,
        suites: suites
            .iter()
            .map(|suite| PackagedSuite {
                suite_name: suite.suite_name.to_string(),
                comparison_path: path_relative_to(workspace_root, &suite.comparison_path),
                summary_path: path_relative_to(workspace_root, &suite.summary_path),
                complete_groups: suite
                    .comparison_export
                    .matrix_groups
                    .iter()
                    .filter(|group| group.complete)
                    .count(),
                total_groups: suite.comparison_export.matrix_groups.len(),
                row_count: suite.comparison_export.rows.len(),
                input_provenance: suite.comparison_export.input_provenance.clone(),
            })
            .collect(),
    };
    let manifest_bytes =
        serde_json::to_vec_pretty(&manifest).context("failed to encode report manifest")?;
    fs::write(&manifest_path, manifest_bytes)
        .await
        .with_context(|| format!("failed to write {}", manifest_path.display()))?;

    Ok(ReportPackageResult {
        output_dir: output_dir.to_path_buf(),
        report_path,
        manifest_path,
        reproducibility_path,
        figure_count: expected_figures.len(),
    })
}

async fn load_packaged_suite(
    comparison_path: &Path,
    expected_suite_name: &'static str,
    expected_group_count: usize,
) -> Result<PackagedSuiteInput> {
    ensure_file_exists(comparison_path, "comparison export").await?;
    let summary_path = summary_path_for_comparison(comparison_path)?;
    ensure_file_exists(&summary_path, "suite summary").await?;

    let bytes = fs::read(comparison_path)
        .await
        .with_context(|| format!("failed to read {}", comparison_path.display()))?;
    let comparison_export: SuiteComparisonExport = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", comparison_path.display()))?;

    ensure!(
        comparison_export.suite_name == expected_suite_name,
        "expected suite {} but {} contains {}",
        expected_suite_name,
        comparison_path.display(),
        comparison_export.suite_name
    );

    let complete_groups = comparison_export
        .matrix_groups
        .iter()
        .filter(|group| group.complete)
        .count();
    ensure!(
        comparison_export.matrix_groups.len() == expected_group_count,
        "expected {} total groups for {} but found {}",
        expected_group_count,
        expected_suite_name,
        comparison_export.matrix_groups.len()
    );
    ensure!(
        complete_groups == expected_group_count,
        "expected {} complete groups for {} but found {}",
        expected_group_count,
        expected_suite_name,
        complete_groups
    );

    Ok(PackagedSuiteInput {
        suite_name: expected_suite_name,
        comparison_path: comparison_path.to_path_buf(),
        summary_path,
        comparison_export,
    })
}

fn expected_figure_inventory() -> Vec<ExpectedFigure> {
    let mut inventory = Vec::new();
    inventory.extend(expected_overview_figures(
        MATRIX_SUITE_NAME,
        MATRIX_OVERVIEW_METRICS,
    ));
    inventory.extend(expected_grouped_figures(
        MATRIX_SUITE_NAME,
        "live_realtime",
        MATRIX_LIVE_METRICS,
    ));
    inventory.extend(expected_grouped_figures(
        MATRIX_SUITE_NAME,
        "vod_realtime",
        MATRIX_VOD_METRICS,
    ));
    inventory.extend(expected_overview_figures(
        FAIRNESS_SUITE_NAME,
        FAIRNESS_OVERVIEW_METRICS,
    ));
    inventory.extend(expected_grouped_figures(
        FAIRNESS_SUITE_NAME,
        "live_realtime",
        FAIRNESS_LIVE_METRICS,
    ));
    inventory
}

fn expected_overview_figures(suite_name: &'static str, metrics: &[&str]) -> Vec<ExpectedFigure> {
    metrics
        .iter()
        .map(|metric| ExpectedFigure {
            suite_name,
            file_name: format!("{}_overview_{}.svg", suite_name, metric),
            category: "overview",
        })
        .collect()
}

fn expected_grouped_figures(
    suite_name: &'static str,
    group_key: &'static str,
    metrics: &[&str],
) -> Vec<ExpectedFigure> {
    metrics
        .iter()
        .map(|metric| ExpectedFigure {
            suite_name,
            file_name: format!("{}_{}_{}.svg", suite_name, group_key, metric),
            category: "grouped",
        })
        .collect()
}

async fn validate_expected_figures(
    figure_dir: &Path,
    expected_figures: &[ExpectedFigure],
) -> Result<()> {
    let mut seen = HashSet::new();
    for figure in expected_figures {
        ensure!(
            seen.insert(figure.file_name.clone()),
            "duplicate expected figure {}",
            figure.file_name
        );
        ensure_file_exists(&figure_dir.join(&figure.file_name), "expected figure").await?;
    }
    Ok(())
}

async fn copy_artifact(
    workspace_root: &Path,
    output_dir: &Path,
    source_path: &Path,
    target_path: &Path,
    role: String,
) -> Result<PackagedArtifact> {
    copy_file(source_path, target_path).await?;
    let provenance = artifact_provenance(workspace_root, source_path).await?;
    Ok(PackagedArtifact {
        role,
        source_path: provenance.path,
        packaged_path: path_relative_to(output_dir, target_path),
        sha256: provenance.sha256,
    })
}

async fn copy_file(source_path: &Path, target_path: &Path) -> Result<()> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::copy(source_path, target_path).await.with_context(|| {
        format!(
            "failed to copy {} to {}",
            source_path.display(),
            target_path.display()
        )
    })?;
    Ok(())
}

async fn artifact_provenance(workspace_root: &Path, path: &Path) -> Result<ArtifactProvenance> {
    let bytes = fs::read(path)
        .await
        .with_context(|| format!("failed to read {}", path.display()))?;
    let digest = Sha256::digest(&bytes);
    Ok(ArtifactProvenance {
        path: path_relative_to(workspace_root, path),
        sha256: hex_digest(&digest),
    })
}

async fn ensure_file_exists(path: &Path, label: &str) -> Result<()> {
    let metadata = fs::metadata(path)
        .await
        .with_context(|| format!("missing {} at {}", label, path.display()))?;
    ensure!(
        metadata.is_file(),
        "{} at {} is not a file",
        label,
        path.display()
    );
    Ok(())
}

async fn ensure_dir_exists(path: &Path, label: &str) -> Result<()> {
    let metadata = fs::metadata(path)
        .await
        .with_context(|| format!("missing {} at {}", label, path.display()))?;
    ensure!(
        metadata.is_dir(),
        "{} at {} is not a directory",
        label,
        path.display()
    );
    Ok(())
}

fn summary_path_for_comparison(comparison_path: &Path) -> Result<PathBuf> {
    let file_name = file_name(comparison_path)?;
    let summary_name = file_name
        .strip_suffix("_comparison.json")
        .map(|prefix| format!("{}_summary.json", prefix))
        .ok_or_else(|| {
            anyhow!(
                "comparison path {} does not end with _comparison.json",
                comparison_path.display()
            )
        })?;
    Ok(comparison_path.with_file_name(summary_name))
}

fn file_name(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
        .ok_or_else(|| anyhow!("path {} has no valid file name", path.display()))
}

fn rewrite_report_links(report: &str) -> String {
    report
        .replace("../results/figures/harness/", "figures/")
        .replace("../results/processed/harness/", "artifacts/")
}

fn build_reproducibility_note(suites: &[PackagedSuiteInput], figure_count: usize) -> String {
    let suite_names = suites
        .iter()
        .map(|suite| format!("- `{}`", suite.suite_name))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "# Reproducibility\n\nThis package was assembled from the frozen Phase 5 and Phase 6 artifacts only.\n\n## Canonical suites\n{suite_names}\n\n## Canonical VPS commands\n\n```bash\ncd /home/leven/quinn-amc\nsudo bash scripts/experiments/run_linux_vps_suite.sh configs/harness/vps_fixed_preset_controller_matrix.json\nsource \"$HOME/.cargo/env\"\ncargo build -p harness\nsudo ./target/debug/harness run-suite --config configs/harness/vps_host_live_coexistence_bbr_guardrail.json\nsudo chown -R \"$USER\":\"$USER\" results\n```\n\n## Canonical figure commands\n\n```powershell\ncargo run -p harness -- plot-suite --comparison results/processed/harness/vps_fixed_preset_controller_matrix_comparison.json --output-dir results/figures/harness\ncargo run -p harness -- plot-suite --comparison results/processed/harness/vps_host_live_coexistence_bbr_guardrail_comparison.json --output-dir results/figures/harness\n```\n\n## Package validation\n\n- expected figure count: `{figure_count}`\n- accepted evidence families: `{}` and `{}`\n- excluded evidence families remain outside this package even if they exist elsewhere under `results/`\n",
        MATRIX_SUITE_NAME, FAIRNESS_SUITE_NAME,
    )
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push_str(&format!("{:02x}", byte));
    }
    output
}

fn path_relative_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::{expected_figure_inventory, rewrite_report_links, summary_path_for_comparison};
    use std::{collections::HashSet, path::Path};

    #[test]
    fn expected_inventory_contains_39_unique_figures() {
        let inventory = expected_figure_inventory();
        assert_eq!(inventory.len(), 39);
        let unique = inventory
            .iter()
            .map(|figure| figure.file_name.clone())
            .collect::<HashSet<_>>();
        assert_eq!(unique.len(), 39);
    }

    #[test]
    fn report_links_are_rewritten_for_packaged_copy() {
        let original = "[throughput](../results/figures/harness/example.svg) and [comparison](../results/processed/harness/example.json)";
        let rewritten = rewrite_report_links(original);
        assert!(rewritten.contains("figures/example.svg"));
        assert!(rewritten.contains("artifacts/example.json"));
    }

    #[test]
    fn comparison_paths_map_to_summary_paths() {
        let summary = summary_path_for_comparison(Path::new(
            "results/processed/harness/vps_fixed_preset_controller_matrix_comparison.json",
        ))
        .expect("summary path");
        assert_eq!(
            summary.file_name().and_then(|name| name.to_str()),
            Some("vps_fixed_preset_controller_matrix_summary.json")
        );
    }
}
