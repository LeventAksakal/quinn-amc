use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use anyhow::{Result, bail};
use demo_client::{BaselineController, Pace, ReplayMode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct SuiteConfig {
    pub suite_name: String,
    pub host: String,
    pub base_port: u16,
    pub cert_path: PathBuf,
    pub replay_manifest: PathBuf,
    pub server_startup_delay_ms: u64,
    pub results_root: PathBuf,
    pub network_scenarios: Vec<NetworkScenario>,
    pub semantic_profile: SemanticProfileConfig,
    pub runs: Vec<RunConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NetworkScenario {
    pub name: String,
    pub kind: NetworkScenarioKind,
    pub description: Option<String>,
    pub rtt_ms: Option<u64>,
    pub loss_percent: Option<f64>,
    pub bandwidth_mbps: Option<u64>,
    pub tc_netem_enabled: bool,
    pub tc_netem: Option<TcNetemConfig>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TcNetemConfig {
    pub interface: String,
    pub delay_jitter_ms: Option<u64>,
    pub limit_packets: Option<u32>,
    pub rate_burst_kbit: Option<u64>,
    pub rate_latency_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkScenarioKind {
    Local,
    LinuxTcNetem,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SemanticProfileConfig {
    pub startup_segments: u64,
    pub startup_importance: ImportanceConfig,
    pub vod_steady_importance: ImportanceConfig,
    pub live_steady_importance: ImportanceConfig,
    pub independent_segment_interval: u64,
    pub dependent_depth: u8,
    pub vod_freshness_window_ms: u64,
    pub live_freshness_window_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportanceConfig {
    Background,
    Normal,
    High,
    Critical,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RunConfig {
    pub name: String,
    #[serde(default)]
    pub controller: BaselineController,
    pub mode: ReplayMode,
    pub pace: Pace,
    pub network_scenario: String,
    pub vod_deadline_slack_ms: Option<u64>,
}

pub fn find_network_scenario<'a>(
    scenarios: &'a [NetworkScenario],
    name: &str,
) -> Option<&'a NetworkScenario> {
    scenarios.iter().find(|scenario| scenario.name == name)
}

pub fn validate_suite_config(config: &SuiteConfig) -> Result<()> {
    let mut errors = Vec::new();

    validate_output_label(&config.suite_name, "suite_name", &mut errors);
    validate_non_empty(&config.host, "host", &mut errors);
    validate_non_empty_path(&config.cert_path, "cert_path", &mut errors);
    validate_non_empty_path(&config.replay_manifest, "replay_manifest", &mut errors);
    validate_non_empty_path(&config.results_root, "results_root", &mut errors);

    if config.base_port == 0 {
        errors.push("base_port must be greater than zero".to_string());
    }
    if config.server_startup_delay_ms == 0 {
        errors.push("server_startup_delay_ms must be greater than zero".to_string());
    }
    if config.semantic_profile.startup_segments == 0 {
        errors.push("semantic_profile.startup_segments must be greater than zero".to_string());
    }

    if config.network_scenarios.is_empty() {
        errors.push("network_scenarios must contain at least one scenario".to_string());
    }
    if config.runs.is_empty() {
        errors.push("runs must contain at least one run".to_string());
    }
    if config.vod_freshness_window_ms() == 0 {
        errors
            .push("semantic_profile.vod_freshness_window_ms must be greater than zero".to_string());
    }
    if config.live_freshness_window_ms() == 0 {
        errors.push(
            "semantic_profile.live_freshness_window_ms must be greater than zero".to_string(),
        );
    }

    if !config.runs.is_empty()
        && config
            .base_port
            .checked_add(config.runs.len().saturating_sub(1) as u16)
            .is_none()
    {
        errors.push(format!(
            "base_port {} overflows the u16 port range for {} configured runs",
            config.base_port,
            config.runs.len()
        ));
    }

    let mut scenario_names = HashSet::new();
    for scenario in &config.network_scenarios {
        validate_output_label(&scenario.name, "network_scenarios[].name", &mut errors);
        if !scenario_names.insert(scenario.name.as_str()) {
            errors.push(format!(
                "duplicate network scenario name '{}' in network_scenarios",
                scenario.name
            ));
        }

        if matches!(scenario.rtt_ms, Some(0)) {
            errors.push(format!(
                "network scenario '{}' must use rtt_ms > 0 when provided",
                scenario.name
            ));
        }
        if let Some(loss_percent) = scenario.loss_percent {
            if !(0.0..=100.0).contains(&loss_percent) {
                errors.push(format!(
                    "network scenario '{}' must use loss_percent in the range 0..=100",
                    scenario.name
                ));
            }
        }
        if matches!(scenario.bandwidth_mbps, Some(0)) {
            errors.push(format!(
                "network scenario '{}' must use bandwidth_mbps > 0 when provided",
                scenario.name
            ));
        }

        match scenario.kind {
            NetworkScenarioKind::Local => {
                if scenario.tc_netem_enabled {
                    errors.push(format!(
                        "local network scenario '{}' cannot enable tc_netem",
                        scenario.name
                    ));
                }
                if scenario.tc_netem.is_some() {
                    errors.push(format!(
                        "local network scenario '{}' cannot include tc_netem settings",
                        scenario.name
                    ));
                }
            }
            NetworkScenarioKind::LinuxTcNetem => {
                if scenario.tc_netem.is_none() {
                    errors.push(format!(
                        "linux_tc_netem scenario '{}' must include tc_netem settings",
                        scenario.name
                    ));
                }
                if let Some(tc_netem) = &scenario.tc_netem {
                    if tc_netem.interface.trim().is_empty() {
                        errors.push(format!(
                            "linux_tc_netem scenario '{}' must provide a non-empty tc_netem.interface",
                            scenario.name
                        ));
                    }
                    if matches!(tc_netem.limit_packets, Some(0)) {
                        errors.push(format!(
                            "linux_tc_netem scenario '{}' must use tc_netem.limit_packets > 0 when provided",
                            scenario.name
                        ));
                    }
                    if matches!(tc_netem.rate_burst_kbit, Some(0)) {
                        errors.push(format!(
                            "linux_tc_netem scenario '{}' must use tc_netem.rate_burst_kbit > 0 when provided",
                            scenario.name
                        ));
                    }
                    if matches!(tc_netem.rate_latency_ms, Some(0)) {
                        errors.push(format!(
                            "linux_tc_netem scenario '{}' must use tc_netem.rate_latency_ms > 0 when provided",
                            scenario.name
                        ));
                    }
                }
            }
        }
    }

    let mut run_names = HashSet::new();
    let mut matrix_cells = HashMap::new();
    let mut comparison_groups = HashMap::new();
    for run in &config.runs {
        validate_output_label(&run.name, "runs[].name", &mut errors);
        if !run_names.insert(run.name.as_str()) {
            errors.push(format!("duplicate run name '{}' in runs", run.name));
        }
        if find_network_scenario(&config.network_scenarios, &run.network_scenario).is_none() {
            errors.push(format!(
                "run '{}' references unknown network scenario '{}'",
                run.name, run.network_scenario
            ));
        }
        if matches!(run.vod_deadline_slack_ms, Some(0)) {
            errors.push(format!(
                "run '{}' must use vod_deadline_slack_ms > 0 when provided",
                run.name
            ));
        }

        let matrix_cell_key = format!(
            "{}|{}|{}|{}",
            controller_label(run.controller),
            replay_mode_label(run.mode),
            pace_label(run.pace),
            run.network_scenario
        );
        if let Some(existing_run) = matrix_cells.insert(matrix_cell_key, run.name.as_str()) {
            errors.push(format!(
                "run '{}' duplicates controller matrix coverage already provided by run '{}' for controller '{}', mode '{}', pace '{}', and network scenario '{}'",
                run.name,
                existing_run,
                controller_label(run.controller),
                replay_mode_label(run.mode),
                pace_label(run.pace),
                run.network_scenario
            ));
        }

        let comparison_group_key = format!(
            "{}|{}|{}",
            replay_mode_label(run.mode),
            pace_label(run.pace),
            run.network_scenario
        );
        match comparison_groups.entry(comparison_group_key) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert((run.name.as_str(), run.vod_deadline_slack_ms));
            }
            std::collections::hash_map::Entry::Occupied(entry) => {
                let (reference_run, reference_slack_ms) = entry.get();
                if *reference_slack_ms != run.vod_deadline_slack_ms {
                    errors.push(format!(
                        "run '{}' must use the same vod_deadline_slack_ms as run '{}' because they share mode '{}', pace '{}', and network scenario '{}'",
                        run.name,
                        reference_run,
                        replay_mode_label(run.mode),
                        pace_label(run.pace),
                        run.network_scenario
                    ));
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        bail!("invalid harness suite config:\n- {}", errors.join("\n- "))
    }
}

fn validate_output_label(value: &str, field_name: &str, errors: &mut Vec<String>) {
    validate_non_empty(value, field_name, errors);
    if value.contains('/') || value.contains('\\') {
        errors.push(format!(
            "{} must not contain path separators because it is used in output file names",
            field_name
        ));
    }
}

fn validate_non_empty(value: &str, field_name: &str, errors: &mut Vec<String>) {
    if value.trim().is_empty() {
        errors.push(format!("{} must not be empty", field_name));
    }
}

fn validate_non_empty_path(path: &std::path::Path, field_name: &str, errors: &mut Vec<String>) {
    if path.as_os_str().is_empty() {
        errors.push(format!("{} must not be empty", field_name));
    }
}

impl SuiteConfig {
    fn vod_freshness_window_ms(&self) -> u64 {
        self.semantic_profile.vod_freshness_window_ms
    }

    fn live_freshness_window_ms(&self) -> u64 {
        self.semantic_profile.live_freshness_window_ms
    }
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

fn pace_label(pace: Pace) -> &'static str {
    match pace {
        Pace::Immediate => "immediate",
        Pace::Realtime => "realtime",
    }
}
