---
marp: true
paginate: true
title: quinn-amc
description: 10-minute presentation on AMC v1 design, experiment choices, and results
---

# quinn-amc

## Semantic-Aware QUIC Congestion Control For Multimedia

- Goal: test whether sender-visible media semantics improve QUIC outcomes
- Baselines: Quinn `new_reno`, `cubic`, `bbr`
- Custom controller: `amc_preview`, the AMC v1 design in this repo
- Main result: AMC helps in the hardest live cells against loss-based baselines, but BBR remains the strongest overall live baseline

---

# Claim Boundary

- This repository is complete at the AMC v1 boundary, not an AMC v2 or BBR-replacement boundary
- Supported claim:
  - AMC v1 improves the hardest constrained live cases relative to `new_reno` and `cubic`
  - BBR remains best overall on live latency-sensitive metrics
  - VOD continuity is stable, but AMC is not a startup-delay winner
  - fairness against BBR is acceptable at the throughput-sharing level
- This is a bounded research result, not a universal claim about semantic transport

---

# Baseline Intuition

- `new_reno`
  - classic loss-based AIMD behavior
  - reacts after congestion becomes visible through loss
- `cubic`
  - still loss-based, but grows more aggressively than Reno
  - often better throughput behavior, but can still build queues badly for live media
- `bbr`
  - model-based controller using bottleneck bandwidth and minimum RTT estimates
  - strongest baseline here because live metrics care heavily about latency, jitter, and freshness

---

# Reference: Controller Selection

```rust
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
}
```

- Baselines come directly from Quinn
- Only `amc_preview` is custom in this repository

---

# AMC v1 Design

- AMC v1 does not replace QUIC with a new transport model
- It stays connection-wide and cwnd-based inside Quinn's congestion-controller interface
- The sender computes a utility signal from media semantics:
  - traffic class: `vod` or `live`
  - importance
  - dependency depth
  - delivery deadline
  - freshness window
  - size
- That utility signal modulates:
  - ACK-driven window growth
  - loss backoff after congestion events

---

# AMC Design Choices

- Live gets higher priority than VOD
  - higher traffic weight
  - more aggressive ACK gain
  - less punitive loss reduction
- Expired or stale units are penalized
- Dependency-blocked units are penalized
- Larger units are penalized relative to smaller urgent units
- The utility signal is smoothed with EWMA
  - more stable than instant switching
  - but less responsive than a richer controller state

### Why that matters

- Good fit for freshness-sensitive live traffic
- Not optimized for fastest VOD startup
- Stronger than generic loss-based control in hard live cells
- Still structurally weaker than BBR on latency control

---

# Reference: Utility Score

```rust
impl UtilityScorer for DefaultUtilityScorer {
  fn score(&self, inputs: &UtilityInputs) -> UtilityScore {
    let importance_weight = match inputs.semantics.importance {
      Importance::Background => 0.25,
      Importance::Normal => 1.0,
      Importance::High => 2.0,
      Importance::Critical => 4.0,
    };

    let traffic_weight = match inputs.semantics.traffic_class {
      TrafficClass::Vod => 1.0,
      TrafficClass::Live => 1.25,
    };

    let dependency_depth_penalty =
      1.0 / (1.0 + f64::from(inputs.semantics.dependency_depth.0));
    let dependency_penalty = if inputs.dependency_ready { 1.0 } else { 0.2 };

    UtilityScore(
      (importance_weight * traffic_weight * dependency_depth_penalty * dependency_penalty)
        / (inputs.semantics.size_bytes.max(1) as f64).sqrt(),
    )
  }
}
```

- Higher importance and live traffic get more weight
- Dependency-blocked and larger units are penalized
- Full implementation also includes deadline and freshness penalties

---

# Reference: Utility To Control Signal

```rust
impl UtilitySignal {
  pub fn from_score_for_traffic_class(
    traffic_class: TrafficClass,
    score: UtilityScore,
  ) -> Self {
    let normalized = match traffic_class {
      TrafficClass::Vod => (score.0 * 96.0).clamp(0.0, 1.0).sqrt(),
      TrafficClass::Live => (score.0 * 128.0).clamp(0.0, 1.0).sqrt(),
    };

    let (ack_gain, loss_reduction_factor) = match traffic_class {
      TrafficClass::Vod => (0.55 + (0.35 * normalized), 0.50 + (0.15 * normalized)),
      TrafficClass::Live => (1.0 + (1.0 * normalized), 0.72 + (0.16 * normalized)),
    };

    Self { traffic_class, score, ack_gain, loss_reduction_factor }
  }
}
```

- Live is intentionally more aggressive than VOD
- This code directly explains why AMC helps live more than VOD

---

# Canonical Experiment Setup

- Final evidence comes from exactly two VPS suites
- Fixed matrix:
  - `configs/harness/vps_fixed_preset_controller_matrix.json`
  - single-flow comparison across workloads, controllers, and fixed network presets
- Fairness guardrail:
  - `configs/harness/vps_host_live_coexistence_bbr_guardrail.json`
  - foreground flow competes with BBR to verify fair bottleneck sharing
- The workflow is intentionally split:
  - fixed matrix uses the docker runner with host-managed `tc` on the server container veth
  - fairness runs directly on the VPS host with `tc` on `lo`

---

# Config Choices

## Controllers

- `new_reno`
- `cubic`
- `bbr`
- `amc_preview`

## Workloads

- `vod`
- `live`

## Fixed network presets

- `wired_clean`
- `wifi_moderate`
- `wifi_unstable`
- `lte_moderate`
- `lte_constrained`

## Why fixed presets

- explicit RTT, loss, bandwidth, jitter, and queue settings
- reproducible comparisons
- easier to defend than dynamic uncontrolled runs

---

# Reference: Canonical Config Excerpt

```json
{
  "name": "lte_constrained",
  "kind": "linux_tc_netem",
  "rtt_ms": 110,
  "loss_percent": 1.5,
  "bandwidth_mbps": 8,
  "tc_netem_enabled": true,
  "tc_netem": {
    "interface": "server-container-host-veth",
    "delay_jitter_ms": 20,
    "limit_packets": 128,
    "rate_burst_kbit": 192,
    "rate_latency_ms": 65
  }
}
```

```json
{
  "startup_segments": 3,
  "startup_importance": "critical",
  "vod_steady_importance": "normal",
  "live_steady_importance": "high",
  "independent_segment_interval": 4,
  "dependent_depth": 1,
  "vod_freshness_window_ms": 30000,
  "live_freshness_window_ms": 1000
}
```

- Fixed network preset plus live-first semantic policy

---

# Hardest Preset And Semantic Profile

## `lte_constrained`

- RTT: `110 ms`
- Loss: `1.5%`
- Bandwidth: `8 Mbps`
- Extra jitter and bounded queue through Linux `tc netem`
- This is the hardest fixed live cell and the main AMC success surface

## Semantic profile choices

- startup segments: `3`
- startup importance: `critical`
- live steady importance: `high`
- VOD steady importance: `normal`
- live freshness window: `1000 ms`
- VOD freshness window: `30000 ms`

### Interpretation

- The design is intentionally live-first
- If semantics help anywhere, they should help most here

---

# Metrics That Matter

## Live

- useful media ratio
- deadline miss rate
- average age of information
- delivery latency
- jitter

## VOD

- startup delay
- rebuffer ratio
- useful media ratio

## Fairness

- foreground throughput share
- throughput ratio
- Jain fairness index

### Why these metrics

- They reflect application quality, not just transport throughput

---

# Reference: Fairness Metric

```rust
pub fn compute_fairness_metrics(
  foreground: &AmcAggregate,
  competitor: &AmcAggregate,
) -> FairnessMetrics {
  let foreground_throughput = foreground.throughput_mbps.max(0.0);
  let competitor_throughput = competitor.throughput_mbps.max(0.0);
  let total = (foreground_throughput + competitor_throughput).max(f64::EPSILON);
  let sum = foreground_throughput + competitor_throughput;
  let sum_sq = foreground_throughput.powi(2) + competitor_throughput.powi(2);

  FairnessMetrics {
    foreground_throughput_share: foreground_throughput / total,
    throughput_ratio: foreground_throughput / competitor_throughput,
    jain_fairness_index: (sum * sum) / (2.0 * sum_sq),
    ..
  }
}
```

- Fairness is defined here as throughput sharing fairness against BBR

---

# Live Results: Usefulness And Deadline Misses

- `wifi_unstable` is AMC's cleanest bounded success case
  - AMC matches BBR on useful media ratio and deadline misses
  - AMC beats `new_reno`
- `lte_constrained` is the main hard case
  - AMC clearly beats `new_reno` and `cubic`
  - BBR still remains best overall

![Live Useful Media Ratio](../results/vps/figures/harness/vps_fixed_preset_controller_matrix_live_realtime_useful_media_ratio.svg)

![Live Deadline Miss Rate](../results/vps/figures/harness/vps_fixed_preset_controller_matrix_live_realtime_deadline_miss_rate.svg)

---

# Live Results: Why BBR Still Leads

- `lte_constrained` live, single-flow:
  - AMC useful media ratio: `0.976`
  - Cubic useful media ratio: `0.833`
  - NewReno useful media ratio: `0.881`
  - BBR useful media ratio: `1.000`
- But the bigger gap is latency behavior:
  - AMC average delivery latency: `242.8 ms`
  - BBR average delivery latency: `33.8 ms`
  - AMC average AoI: `243.6 ms`
  - BBR average AoI: `34.1 ms`

### Interpretation

- AMC improves over loss-based baselines because it knows what is urgent
- BBR still wins because it is structurally better at controlling latency and queue buildup

![Live Average Age Of Information](../results/vps/figures/harness/vps_fixed_preset_controller_matrix_live_realtime_average_age_of_information_ms.svg)

---

# VOD Results: Stable, But Not A Startup Winner

- VOD continuity stays stable in the constrained presets
  - useful media ratio stays `1.0`
  - rebuffer ratio stays `0.0`
- AMC is slower to start than BBR
  - `wifi_unstable`: AMC `2160 ms`, BBR `1239 ms`
  - `lte_constrained`: AMC `2412 ms`, BBR `2018 ms`

### Why

- AMC intentionally treats VOD more conservatively than live
- lower priority, lower aggressiveness, tighter class cap than live traffic

![VOD Startup Delay](../results/vps/figures/harness/vps_fixed_preset_controller_matrix_vod_realtime_vod_startup_delay_ms.svg)

---

# Fairness Guardrail

- AMC does not get its bounded live gains by grabbing an unfair share from BBR
- In the fairness suite, throughput sharing remains near `0.5`
- Jain fairness stays effectively `1.0`

## AMC foreground numbers

- `lte_constrained`: throughput share `0.5002`, Jain fairness `0.9999998`
- `wifi_unstable`: throughput share `0.4998`, Jain fairness `0.9999999`

### Interpretation

- Fairness claim is narrow and specific
- Acceptable bottleneck sharing does not mean AMC matches BBR on freshness under competition

![Fairness Foreground Throughput Share](../results/vps/figures/harness/vps_host_live_coexistence_bbr_guardrail_live_realtime_fairness_foreground_throughput_share.svg)

---

# Takeaways

- AMC v1 is a useful step, not a final controller
- Sender-visible semantics help most in the hardest constrained live cases
- AMC improves over Quinn's loss-based baselines where freshness matters most
- BBR remains the strongest overall live baseline in this repository
- VOD startup remains a known weakness for AMC v1
- Fairness against BBR is acceptable at the throughput-sharing level

## Bottom line

- Semantic awareness helps
- But AMC v1 does not yet close the full gap to BBR
