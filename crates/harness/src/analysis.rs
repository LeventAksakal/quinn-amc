use std::{
    collections::HashMap,
    path::Path,
    time::Duration,
};

use amc_core::{
    DefaultUtilityScorer, Importance, MediaSemantics, TrafficClass, UtilityInputs, UtilityScorer,
};
use anyhow::{Context, Result};
use demo_client::{BaselineController, ReplayMode};
use demo_server::{SegmentKind, TransferReport};
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::config::{
    ImportanceConfig, NetworkScenario, NetworkScenarioKind, RunConfig, SemanticProfileConfig,
};

#[derive(Debug, Deserialize)]
pub struct ReplayManifest {
    pub asset_name: String,
    #[serde(default)]
    pub semantic_defaults: ReplaySemanticDefaults,
    pub segments: Vec<ReplaySegment>,
}

#[derive(Debug, Deserialize)]
pub struct ReplaySegment {
    pub sequence: u64,
    pub relative_path: String,
    pub start_time_ms: u64,
    pub duration_ms: u64,
    pub size_bytes: u64,
    #[serde(default)]
    pub semantic_hint: ReplaySemanticHint,
}

#[derive(Debug, Default, Deserialize)]
pub struct ReplaySemanticDefaults {
    pub startup_segment_count: Option<u64>,
    pub default_dependency_depth_hint: Option<u8>,
    pub default_freshness_window_ms: Option<u64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ReplaySemanticHint {
    pub importance_hint: Option<ImportanceConfig>,
    pub dependency_depth_hint: Option<u8>,
    pub independent: Option<bool>,
    pub freshness_window_ms: Option<u64>,
    pub priority_label: Option<String>,
    pub size_tier: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SuiteSummary {
    pub suite_name: String,
    pub replay_manifest: String,
    pub network_scenarios: Vec<NetworkScenario>,
    pub runs: Vec<RunOutcome>,
}

#[derive(Debug, Serialize)]
pub struct RunOutcome {
    pub name: String,
    pub controller: BaselineController,
    pub mode: ReplayMode,
    pub pace: demo_client::Pace,
    pub server: String,
    pub report_path: String,
    pub network_scenario: NetworkScenario,
    pub amc_analysis_path: String,
    pub amc_aggregate: AmcAggregate,
    pub summary: demo_server::TransferSummary,
}

#[derive(Debug, Serialize)]
pub struct SuiteComparisonExport {
    pub suite_name: String,
    pub replay_manifest: String,
    pub matrix_groups: Vec<ComparisonGroup>,
    pub rows: Vec<ComparisonRow>,
}

#[derive(Debug, Serialize)]
pub struct ComparisonGroup {
    pub comparison_cell: String,
    pub mode: ReplayMode,
    pub pace: demo_client::Pace,
    pub network_scenario: String,
    pub network_kind: NetworkScenarioKind,
    pub tc_netem_enabled: bool,
    pub controller_runs: Vec<ControllerRunRef>,
}

#[derive(Debug, Serialize)]
pub struct ControllerRunRef {
    pub controller: BaselineController,
    pub run_name: String,
    pub report_path: String,
    pub amc_analysis_path: String,
}

#[derive(Debug, Serialize)]
pub struct ComparisonRow {
    pub run_name: String,
    pub comparison_cell: String,
    pub controller: BaselineController,
    pub mode: ReplayMode,
    pub pace: demo_client::Pace,
    pub network_scenario: String,
    pub network_kind: NetworkScenarioKind,
    pub tc_netem_enabled: bool,
    pub rtt_ms: Option<u64>,
    pub loss_percent: Option<f64>,
    pub bandwidth_mbps: Option<u64>,
    pub report_path: String,
    pub amc_analysis_path: String,
    pub segments_received: usize,
    pub media_segments_received: usize,
    pub useful_media_segments: usize,
    pub useful_media_ratio: f64,
    pub late_media_segments: usize,
    pub late_media_ratio: f64,
    pub max_observed_lateness_ms: i64,
    pub total_payload_bytes: u64,
    pub amc_runtime_samples: usize,
    pub average_media_utility_score: f64,
    pub useful_media_utility_sum: f64,
    pub dependency_blocked_media_units: usize,
}

#[derive(Debug, Serialize)]
pub struct AmcRunAnalysis {
    pub run_name: String,
    pub controller: BaselineController,
    pub asset_name: String,
    pub network_scenario: NetworkScenario,
    pub semantic_profile: SemanticProfileConfig,
    pub aggregate: AmcAggregate,
    pub units: Vec<AmcUnitAnalysis>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AmcAggregate {
    pub units_scored: usize,
    pub media_units_scored: usize,
    pub useful_media_units: usize,
    pub zero_score_media_units: usize,
    pub dependency_blocked_media_units: usize,
    pub average_media_utility_score: f64,
    pub useful_media_utility_sum: f64,
    pub max_media_utility_score: f64,
    pub min_media_utility_score: f64,
}

#[derive(Debug, Serialize)]
pub struct AmcUnitAnalysis {
    pub asset_name: String,
    pub sequence: u64,
    pub segment_kind: &'static str,
    pub traffic_class: &'static str,
    pub importance: &'static str,
    pub dependency_depth: u8,
    pub dependency_ready: bool,
    pub useful: bool,
    pub semantic_source: &'static str,
    pub priority_label: Option<String>,
    pub size_tier: Option<String>,
    pub payload_len: u64,
    pub segment_path: String,
    pub delivery_deadline_ms: Option<u64>,
    pub freshness_window_ms: Option<u64>,
    pub queue_delay_ms: u64,
    pub estimated_rtt_ms: u64,
    pub utility_score: f64,
}

pub async fn load_replay_manifest(path: &Path) -> Result<ReplayManifest> {
    let bytes = fs::read(path)
        .await
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse replay manifest {}", path.display()))
}

pub async fn load_transfer_report(path: &Path) -> Result<TransferReport> {
    let bytes = fs::read(path)
        .await
        .with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse transfer report {}", path.display()))
}

pub async fn write_amc_analysis(path: &Path, analysis: &AmcRunAnalysis) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(analysis).context("failed to encode AMC analysis")?;
    fs::write(path, bytes)
        .await
        .with_context(|| format!("failed to write {}", path.display()))
}

pub fn analyze_report(
    run: &RunConfig,
    network_scenario: &NetworkScenario,
    semantic_profile: &SemanticProfileConfig,
    replay_manifest: &ReplayManifest,
    report: &TransferReport,
) -> AmcRunAnalysis {
    let scorer = DefaultUtilityScorer;
    let mut units = Vec::with_capacity(report.observations.len());
    let mut media_units_scored = 0usize;
    let mut useful_media_units = 0usize;
    let mut zero_score_media_units = 0usize;
    let mut dependency_blocked_media_units = 0usize;
    let mut utility_sum = 0.0f64;
    let mut useful_media_utility_sum = 0.0f64;
    let mut max_media_utility_score = f64::NEG_INFINITY;
    let mut min_media_utility_score = f64::INFINITY;
    let mut previous_media_useful = true;
    let segment_index: HashMap<u64, &ReplaySegment> = replay_manifest
        .segments
        .iter()
        .map(|segment| (segment.sequence, segment))
        .collect();

    for observation in &report.observations {
        let traffic_class = match run.mode {
            ReplayMode::Vod => TrafficClass::Vod,
            ReplayMode::Live => TrafficClass::Live,
        };
        let manifest_segment = segment_index.get(&observation.sequence).copied();
        let semantic_hint = manifest_segment
            .map(|segment| &segment.semantic_hint)
            .filter(|hint| {
                hint.importance_hint.is_some()
                    || hint.dependency_depth_hint.is_some()
                    || hint.independent.is_some()
                    || hint.freshness_window_ms.is_some()
            });
        let semantic_source = if semantic_hint.is_some() {
            "replay_manifest"
        } else {
            "harness_fallback"
        };
        let manifest_start_time_ms = manifest_segment
            .map(|segment| segment.start_time_ms)
            .unwrap_or(observation.start_time_ms);
        let manifest_duration_ms = manifest_segment
            .map(|segment| segment.duration_ms)
            .unwrap_or(observation.duration_ms);

        let importance = derive_importance(
            observation.sequence,
            observation.kind,
            run.mode,
            semantic_profile,
            replay_manifest,
            semantic_hint,
        );
        let dependency_depth = derive_dependency_depth(
            observation.sequence,
            observation.kind,
            semantic_profile,
            replay_manifest,
            semantic_hint,
        );
        let dependency_ready = dependency_depth == 0 || previous_media_useful;
        let delivery_deadline_ms = match observation.kind {
            SegmentKind::Init => None,
            SegmentKind::Media => Some(observation.deadline_ms.saturating_sub(manifest_start_time_ms)),
        };
        let freshness_window_ms = derive_freshness_window_ms(
            observation.kind,
            run.mode,
            semantic_profile,
            replay_manifest,
            semantic_hint,
        );
        let queue_delay_ms = observation
            .client_send_elapsed_ms
            .saturating_sub(manifest_start_time_ms);
        let estimated_rtt_ms = network_scenario.rtt_ms.unwrap_or_else(|| {
            observation
                .server_receive_elapsed_ms
                .saturating_sub(observation.client_send_elapsed_ms)
        });
        let payload_len = manifest_segment
            .map(|segment| segment.size_bytes)
            .unwrap_or(observation.payload_len);
        let segment_path = manifest_segment
            .map(|segment| segment.relative_path.clone())
            .unwrap_or_else(|| observation.segment_path.clone());

        let mut semantics = MediaSemantics::new(traffic_class, importance, payload_len)
            .with_dependency_depth(dependency_depth);
        if let Some(deadline_ms) = delivery_deadline_ms {
            semantics = semantics.with_delivery_deadline(Duration::from_millis(deadline_ms));
        }
        if let Some(freshness_window_ms) = freshness_window_ms {
            semantics = semantics.with_freshness_window(Duration::from_millis(freshness_window_ms));
        }
        if delivery_deadline_ms.is_none() && manifest_duration_ms > 0 {
            semantics = semantics.with_freshness_window(Duration::from_millis(manifest_duration_ms));
        }

        let utility_score = scorer
            .score(&UtilityInputs {
                semantics,
                queue_delay: Duration::from_millis(queue_delay_ms),
                estimated_rtt: Duration::from_millis(estimated_rtt_ms),
                dependency_ready,
            })
            .0;

        if matches!(observation.kind, SegmentKind::Media) {
            media_units_scored += 1;
            if observation.useful {
                useful_media_units += 1;
                useful_media_utility_sum += utility_score;
            }
            if utility_score == 0.0 {
                zero_score_media_units += 1;
            }
            if !dependency_ready {
                dependency_blocked_media_units += 1;
            }
            utility_sum += utility_score;
            max_media_utility_score = max_media_utility_score.max(utility_score);
            min_media_utility_score = min_media_utility_score.min(utility_score);
            previous_media_useful = observation.useful;
        }

        units.push(AmcUnitAnalysis {
            asset_name: observation.asset_name.clone(),
            sequence: observation.sequence,
            segment_kind: segment_kind_label(observation.kind),
            traffic_class: traffic_class_label(traffic_class),
            importance: importance_label(importance),
            dependency_depth,
            dependency_ready,
            useful: observation.useful,
            semantic_source,
            priority_label: semantic_hint.and_then(|hint| hint.priority_label.clone()),
            size_tier: semantic_hint.and_then(|hint| hint.size_tier.clone()),
            payload_len,
            segment_path,
            delivery_deadline_ms,
            freshness_window_ms,
            queue_delay_ms,
            estimated_rtt_ms,
            utility_score,
        });
    }

    let aggregate = AmcAggregate {
        units_scored: units.len(),
        media_units_scored,
        useful_media_units,
        zero_score_media_units,
        dependency_blocked_media_units,
        average_media_utility_score: if media_units_scored == 0 {
            0.0
        } else {
            utility_sum / media_units_scored as f64
        },
        useful_media_utility_sum,
        max_media_utility_score: if media_units_scored == 0 {
            0.0
        } else {
            max_media_utility_score
        },
        min_media_utility_score: if media_units_scored == 0 {
            0.0
        } else {
            min_media_utility_score
        },
    };

    AmcRunAnalysis {
        run_name: run.name.clone(),
        controller: run.controller,
        asset_name: replay_manifest.asset_name.clone(),
        network_scenario: network_scenario.clone(),
        semantic_profile: semantic_profile.clone(),
        aggregate,
        units,
    }
}

fn derive_importance(
    sequence: u64,
    kind: SegmentKind,
    mode: ReplayMode,
    profile: &SemanticProfileConfig,
    replay_manifest: &ReplayManifest,
    semantic_hint: Option<&ReplaySemanticHint>,
) -> Importance {
    if matches!(kind, SegmentKind::Init) {
        return Importance::Critical;
    }

    if let Some(importance_hint) = semantic_hint.and_then(|hint| hint.importance_hint) {
        return importance_from_config(importance_hint);
    }

    if sequence
        <= replay_manifest
            .semantic_defaults
            .startup_segment_count
            .unwrap_or(profile.startup_segments)
    {
        return importance_from_config(profile.startup_importance);
    }

    match mode {
        ReplayMode::Vod => importance_from_config(profile.vod_steady_importance),
        ReplayMode::Live => importance_from_config(profile.live_steady_importance),
    }
}

fn derive_dependency_depth(
    sequence: u64,
    kind: SegmentKind,
    profile: &SemanticProfileConfig,
    replay_manifest: &ReplayManifest,
    semantic_hint: Option<&ReplaySemanticHint>,
) -> u8 {
    if matches!(kind, SegmentKind::Init) {
        return 0;
    }

    if let Some(independent) = semantic_hint.and_then(|hint| hint.independent) {
        if independent {
            return 0;
        }
    }
    if let Some(depth) = semantic_hint.and_then(|hint| hint.dependency_depth_hint) {
        return depth;
    }

    let default_depth = replay_manifest
        .semantic_defaults
        .default_dependency_depth_hint
        .unwrap_or(profile.dependent_depth);
    if profile.independent_segment_interval == 0 {
        return default_depth;
    }

    if (sequence - 1) % profile.independent_segment_interval == 0 {
        0
    } else {
        default_depth
    }
}

fn derive_freshness_window_ms(
    kind: SegmentKind,
    mode: ReplayMode,
    profile: &SemanticProfileConfig,
    replay_manifest: &ReplayManifest,
    semantic_hint: Option<&ReplaySemanticHint>,
) -> Option<u64> {
    if matches!(kind, SegmentKind::Init) {
        return None;
    }

    let base_window = semantic_hint
        .and_then(|hint| hint.freshness_window_ms)
        .or(replay_manifest.semantic_defaults.default_freshness_window_ms);

    Some(match mode {
        ReplayMode::Vod => base_window
            .unwrap_or(profile.vod_freshness_window_ms)
            .max(profile.vod_freshness_window_ms),
        ReplayMode::Live => base_window
            .unwrap_or(profile.live_freshness_window_ms)
            .min(profile.live_freshness_window_ms),
    })
}

fn importance_from_config(importance: ImportanceConfig) -> Importance {
    match importance {
        ImportanceConfig::Background => Importance::Background,
        ImportanceConfig::Normal => Importance::Normal,
        ImportanceConfig::High => Importance::High,
        ImportanceConfig::Critical => Importance::Critical,
    }
}

fn segment_kind_label(kind: SegmentKind) -> &'static str {
    match kind {
        SegmentKind::Init => "init",
        SegmentKind::Media => "media",
    }
}

fn traffic_class_label(traffic_class: TrafficClass) -> &'static str {
    match traffic_class {
        TrafficClass::Vod => "vod",
        TrafficClass::Live => "live",
    }
}

fn importance_label(importance: Importance) -> &'static str {
    match importance {
        Importance::Background => "background",
        Importance::Normal => "normal",
        Importance::High => "high",
        Importance::Critical => "critical",
    }
}

pub async fn write_comparison_export(path: &Path, export: &SuiteComparisonExport) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(export).context("failed to encode harness comparison export")?;
    fs::write(path, bytes)
        .await
        .with_context(|| format!("failed to write {}", path.display()))
}

pub fn build_suite_comparison_export(
    suite_name: &str,
    replay_manifest: &str,
    runs: &[RunOutcome],
) -> SuiteComparisonExport {
    let mut ordered_runs = runs.iter().collect::<Vec<_>>();
    ordered_runs.sort_by(|left, right| comparison_sort_key(left).cmp(&comparison_sort_key(right)));

    let mut grouped_runs: HashMap<String, Vec<&RunOutcome>> = HashMap::new();
    for run in &ordered_runs {
        grouped_runs
            .entry(comparison_cell(run))
            .or_default()
            .push(*run);
    }

    let mut matrix_groups = grouped_runs
        .into_values()
        .map(ComparisonGroup::from_run_outcomes)
        .collect::<Vec<_>>();
    matrix_groups.sort_by(|left, right| left.comparison_cell.cmp(&right.comparison_cell));

    let rows = ordered_runs
        .into_iter()
        .map(ComparisonRow::from_run_outcome)
        .collect();

    SuiteComparisonExport {
        suite_name: suite_name.to_string(),
        replay_manifest: replay_manifest.to_string(),
        matrix_groups,
        rows,
    }
}

impl ComparisonGroup {
    fn from_run_outcomes(runs: Vec<&RunOutcome>) -> Self {
        let first_run = runs.first().expect("comparison group requires at least one run");
        let comparison_cell = comparison_cell(first_run);
        let mode = first_run.mode;
        let pace = first_run.pace;
        let network_scenario = first_run.network_scenario.name.clone();
        let network_kind = first_run.network_scenario.kind;
        let tc_netem_enabled = first_run.network_scenario.tc_netem_enabled;
        let mut controller_runs = runs
            .into_iter()
            .map(|run| ControllerRunRef {
                controller: run.controller,
                run_name: run.name.clone(),
                report_path: run.report_path.clone(),
                amc_analysis_path: run.amc_analysis_path.clone(),
            })
            .collect::<Vec<_>>();
        controller_runs.sort_by(|left, right| {
            controller_label(left.controller)
                .cmp(controller_label(right.controller))
                .then_with(|| left.run_name.cmp(&right.run_name))
        });

        Self {
            comparison_cell,
            mode,
            pace,
            network_scenario,
            network_kind,
            tc_netem_enabled,
            controller_runs,
        }
    }
}

impl ComparisonRow {
    fn from_run_outcome(run: &RunOutcome) -> Self {
        let media_segments_received = run.summary.media_segments_received;
        let useful_media_ratio = ratio(run.summary.useful_media_segments, media_segments_received);
        let late_media_ratio = ratio(run.summary.late_media_segments, media_segments_received);

        Self {
            run_name: run.name.clone(),
            comparison_cell: comparison_cell(run),
            controller: run.controller,
            mode: run.mode,
            pace: run.pace,
            network_scenario: run.network_scenario.name.clone(),
            network_kind: run.network_scenario.kind,
            tc_netem_enabled: run.network_scenario.tc_netem_enabled,
            rtt_ms: run.network_scenario.rtt_ms,
            loss_percent: run.network_scenario.loss_percent,
            bandwidth_mbps: run.network_scenario.bandwidth_mbps,
            report_path: run.report_path.clone(),
            amc_analysis_path: run.amc_analysis_path.clone(),
            segments_received: run.summary.segments_received,
            media_segments_received,
            useful_media_segments: run.summary.useful_media_segments,
            useful_media_ratio,
            late_media_segments: run.summary.late_media_segments,
            late_media_ratio,
            max_observed_lateness_ms: run.summary.max_observed_lateness_ms,
            total_payload_bytes: run.summary.total_payload_bytes,
            amc_runtime_samples: run.summary.amc_runtime_samples,
            average_media_utility_score: run.amc_aggregate.average_media_utility_score,
            useful_media_utility_sum: run.amc_aggregate.useful_media_utility_sum,
            dependency_blocked_media_units: run.amc_aggregate.dependency_blocked_media_units,
        }
    }
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn comparison_sort_key(run: &RunOutcome) -> String {
    format!(
        "{}|{}|{}|{}|{}",
        network_kind_label(run.network_scenario.kind),
        run.network_scenario.name,
        replay_mode_label(run.mode),
        pace_label(run.pace),
        controller_label(run.controller)
    )
}

fn comparison_cell(run: &RunOutcome) -> String {
    format!(
        "{}|{}|{}",
        run.network_scenario.name,
        replay_mode_label(run.mode),
        pace_label(run.pace)
    )
}

fn controller_label(controller: BaselineController) -> &'static str {
    match controller {
        BaselineController::AmcPreview => "amc_preview",
        BaselineController::Bbr => "bbr",
        BaselineController::Cubic => "cubic",
        BaselineController::NewReno => "new_reno",
    }
}

fn replay_mode_label(mode: ReplayMode) -> &'static str {
    match mode {
        ReplayMode::Vod => "vod",
        ReplayMode::Live => "live",
    }
}

fn pace_label(pace: demo_client::Pace) -> &'static str {
    match pace {
        demo_client::Pace::Immediate => "immediate",
        demo_client::Pace::Realtime => "realtime",
    }
}

fn network_kind_label(kind: NetworkScenarioKind) -> &'static str {
    match kind {
        NetworkScenarioKind::Local => "local",
        NetworkScenarioKind::LinuxTcNetem => "linux_tc_netem",
    }
}