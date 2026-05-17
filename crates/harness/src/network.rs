#[cfg(target_os = "linux")]
use std::process::Command;

#[cfg(target_os = "linux")]
use anyhow::bail;
#[cfg(target_os = "linux")]
use anyhow::{Context, Result, anyhow};
#[cfg(not(target_os = "linux"))]
use anyhow::{Result, bail};
use tracing::{info, warn};

#[cfg(target_os = "linux")]
use crate::config::TcNetemConfig;
use crate::config::{NetworkScenario, NetworkScenarioKind};

pub struct NetworkControlGuard {
    scenario_name: String,
    interface: Option<String>,
    active: bool,
}

impl NetworkControlGuard {
    fn inactive(scenario_name: &str) -> Self {
        Self {
            scenario_name: scenario_name.to_string(),
            interface: None,
            active: false,
        }
    }
}

impl Drop for NetworkControlGuard {
    fn drop(&mut self) {
        if !self.active {
            return;
        }

        if let Some(interface) = &self.interface {
            if let Err(error) = cleanup_tc(interface) {
                warn!(scenario = %self.scenario_name, interface = %interface, error = %error, "failed to clean up tc qdisc");
            }
        }
    }
}

pub fn validate_network_scenario_for_run(scenario: &NetworkScenario) -> Result<()> {
    match scenario.kind {
        NetworkScenarioKind::Local => Ok(()),
        NetworkScenarioKind::LinuxTcNetem => validate_linux_tc_netem_requirements(scenario),
    }
}

pub fn apply_network_scenario(scenario: &NetworkScenario) -> Result<NetworkControlGuard> {
    match scenario.kind {
        NetworkScenarioKind::Local => {
            info!(scenario = %scenario.name, "network scenario is local; no tc setup applied");
            Ok(NetworkControlGuard::inactive(&scenario.name))
        }
        NetworkScenarioKind::LinuxTcNetem => apply_linux_tc_netem(scenario),
    }
}

fn apply_linux_tc_netem(scenario: &NetworkScenario) -> Result<NetworkControlGuard> {
    validate_linux_tc_netem_requirements(scenario)?;

    #[cfg(not(target_os = "linux"))]
    {
        let _ = scenario;
        unreachable!("linux tc netem requirements validation should fail on non-Linux targets")
    }

    #[cfg(target_os = "linux")]
    {
        let tc = scenario
            .tc_netem
            .as_ref()
            .expect("validated tc_netem config");
        apply_tc_root(scenario, tc)?;
        apply_optional_tbf(scenario, tc)?;

        info!(
            scenario = %scenario.name,
            interface = %tc.interface,
            rtt_ms = ?scenario.rtt_ms,
            loss_percent = ?scenario.loss_percent,
            bandwidth_mbps = ?scenario.bandwidth_mbps,
            "applied linux tc netem scenario"
        );

        Ok(NetworkControlGuard {
            scenario_name: scenario.name.clone(),
            interface: Some(tc.interface.clone()),
            active: true,
        })
    }
}

fn validate_linux_tc_netem_requirements(scenario: &NetworkScenario) -> Result<()> {
    if !scenario.tc_netem_enabled {
        bail!(
            "scenario {} is marked linux_tc_netem but tc_netem_enabled=false; enabling this run would skip shaping",
            scenario.name
        );
    }

    #[cfg(not(target_os = "linux"))]
    {
        bail!(
            "scenario {} requires Linux tc netem but this harness binary is not running on Linux",
            scenario.name
        );
    }

    #[cfg(target_os = "linux")]
    {
        let tc = scenario.tc_netem.as_ref().with_context(|| {
            format!(
                "scenario {} is missing tc_netem configuration",
                scenario.name
            )
        })?;
        if tc.interface.trim().is_empty() {
            bail!(
                "scenario {} must provide a non-empty tc_netem.interface for Linux tc netem",
                scenario.name
            );
        }

        ensure_tc_command_available()?;
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn ensure_tc_command_available() -> Result<()> {
    let output = Command::new("tc")
        .arg("-V")
        .output()
        .context("failed to execute tc -V for harness network preflight")?;
    if output.status.success() {
        Ok(())
    } else {
        Err(anyhow!(
            "tc -V failed during harness network preflight: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(target_os = "linux")]
fn apply_tc_root(scenario: &NetworkScenario, tc: &TcNetemConfig) -> Result<()> {
    let mut args = vec![
        "qdisc".to_string(),
        "replace".to_string(),
        "dev".to_string(),
        tc.interface.clone(),
        "root".to_string(),
        "handle".to_string(),
        "1:".to_string(),
        "netem".to_string(),
    ];

    if let Some(rtt_ms) = scenario.rtt_ms {
        let one_way_delay_ms = rtt_ms.max(1) / 2 + u64::from(rtt_ms == 1);
        args.push("delay".to_string());
        args.push(format!("{}ms", one_way_delay_ms.max(1)));
        if let Some(delay_jitter_ms) = tc.delay_jitter_ms {
            args.push(format!("{}ms", delay_jitter_ms));
        }
    }

    if let Some(loss_percent) = scenario.loss_percent {
        if loss_percent > 0.0 {
            args.push("loss".to_string());
            args.push(format!("{}%", loss_percent));
        }
    }

    if let Some(limit_packets) = tc.limit_packets {
        args.push("limit".to_string());
        args.push(limit_packets.to_string());
    }

    run_tc_command(&args)
}

#[cfg(target_os = "linux")]
fn apply_optional_tbf(scenario: &NetworkScenario, tc: &TcNetemConfig) -> Result<()> {
    let Some(bandwidth_mbps) = scenario.bandwidth_mbps else {
        return Ok(());
    };

    let burst_kbit = tc.rate_burst_kbit.unwrap_or((bandwidth_mbps * 32).max(32));
    let latency_ms = tc.rate_latency_ms.unwrap_or(50);
    let args = vec![
        "qdisc".to_string(),
        "replace".to_string(),
        "dev".to_string(),
        tc.interface.clone(),
        "parent".to_string(),
        "1:1".to_string(),
        "handle".to_string(),
        "10:".to_string(),
        "tbf".to_string(),
        "rate".to_string(),
        format!("{}mbit", bandwidth_mbps),
        "burst".to_string(),
        format!("{}kbit", burst_kbit),
        "latency".to_string(),
        format!("{}ms", latency_ms),
    ];

    run_tc_command(&args)
}

#[cfg(target_os = "linux")]
fn cleanup_tc(interface: &str) -> Result<()> {
    let args = vec![
        "qdisc".to_string(),
        "del".to_string(),
        "dev".to_string(),
        interface.to_string(),
        "root".to_string(),
    ];
    match run_tc_command(&args) {
        Ok(()) => {
            info!(interface = %interface, "removed tc qdisc from interface");
            Ok(())
        }
        Err(error) => {
            let message = error.to_string();
            if message.contains("No such file or directory")
                || message.contains("Cannot delete qdisc with handle of zero")
            {
                Ok(())
            } else {
                Err(error)
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn cleanup_tc(_interface: &str) -> Result<()> {
    Ok(())
}

#[cfg(target_os = "linux")]
fn run_tc_command(args: &[String]) -> Result<()> {
    let output = Command::new("tc")
        .args(args)
        .output()
        .with_context(|| format!("failed to execute tc {}", args.join(" ")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(anyhow!(
            "tc {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}
