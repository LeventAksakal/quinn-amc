# Presenter Notes

## Purpose

These notes are written as a speaker script for presenting the repository as it exists today, at the frozen AMC v1 boundary. The baseline algorithm descriptions are standard transport-level summaries. The repository-specific claims, experiment setup, code excerpts, and result interpretations are grounded in this workspace.

Use this as a word-for-word script if needed, or trim sections into slide speaker notes.

## One-Sentence Thesis

AMC v1 in this repository is not a BBR replacement; it is a semantic-aware congestion-control augment that improves the hardest constrained live cells relative to Quinn's loss-based baselines, while BBR remains the strongest overall live baseline on latency-sensitive metrics.

---

## Opening Script

Say:

"Today I am presenting a Rust research workspace called quinn-amc. The project asks a narrow question: if the sender knows something about media urgency, dependencies, freshness, and importance, can that information improve QUIC application outcomes compared with standard congestion-control baselines?"

"The answer from this repository is deliberately bounded. AMC v1 improves the hardest constrained live cases relative to NewReno and Cubic. BBR remains the strongest overall live baseline. VOD continuity remains stable, but AMC is not a startup-delay winner. Fairness against BBR is acceptable at the throughput-sharing level."

"That scope boundary matters, because this repository is not claiming AMC v2, not claiming broad BBR parity, and not claiming that semantic awareness by itself solves every congestion-control problem."

---

## Section 1: What Congestion Control Is Doing Here

Say:

"At a high level, congestion control is deciding how aggressively the sender can inject data into the network without causing persistent queue growth, excessive loss, or unstable behavior. In QUIC, that logic operates per connection, and it interacts with acknowledgments, loss signals, RTT estimates, and pacing or congestion-window limits."

"For this repository, the key comparison is between three standard Quinn baseline controllers, NewReno, Cubic, and BBR, and one custom controller, AMC preview, which is the AMC v1 implementation."

The baseline selection is wired in the demo client here:

```rust
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum BaselineController {
    #[value(alias = "amc_preview")]
    AmcPreview,
    Bbr,
    #[default]
    Cubic,
    #[value(alias = "new_reno")]
    NewReno,
}

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

Say:

"That snippet is important because it shows that the baselines are not reimplemented here. The workspace is using Quinn's built-in BBR, Cubic, and NewReno controllers directly, and only AMC preview is custom. That keeps the comparison cleaner."

---

## Section 2: NewReno, Cubic, And BBR In Plain Language

### NewReno

Say:

"NewReno is the classic loss-based additive-increase, multiplicative-decrease controller. In slow start it increases aggressively until it finds congestion. After that it grows the congestion window roughly linearly, and when loss occurs it cuts the window down."

"The practical effect is that NewReno is simple and conservative, but it tends to interpret loss as the primary sign of congestion. In a multimedia setting, especially under constrained or jittery paths, that can leave the controller reacting after the path has already become bad for timely media delivery."

"So NewReno is a reasonable baseline, but it is not semantics-aware, and it is not designed around freshness-sensitive media utility."

### Cubic

Say:

"Cubic is also fundamentally loss-based, but instead of NewReno's linear congestion-avoidance growth, Cubic uses a cubic growth function for the congestion window. The goal is to recover capacity faster and be less RTT-sensitive than Reno-style growth."

"In practice, Cubic usually probes available bandwidth more aggressively than NewReno. That often helps throughput, but it can still build queues and produce timing behavior that is not ideal for live media. So it can outperform NewReno in many bulk-transfer situations, yet still struggle on freshness-sensitive workloads when queue growth starts to dominate."

### BBR

Say:

"BBR is different in kind. BBR is model-based, not purely loss-based. It tries to estimate bottleneck bandwidth and minimum RTT, then operate around that model using pacing and a congestion-window cap. Instead of waiting for loss to tell it what happened, it actively probes for bandwidth and works to keep the path near its estimated operating point."

"That is why BBR is such a strong baseline for this repository. The main live metrics here, age of information, delivery latency, jitter, and deadline misses, care a lot about queue control and timely delivery. BBR often wins those metrics because it avoids some of the excess queue buildup that loss-based controllers tolerate before backing off."

"So before looking at AMC, the right baseline intuition is: NewReno and Cubic are loss-triggered window controllers, while BBR is a path-model controller with strong latency behavior."

---

## Section 3: What AMC v1 Is Actually Doing

Say:

"AMC stands for application or semantic-aware multimedia congestion control in this workspace. The important qualifier is version one. AMC v1 is intentionally narrow. It does not feed per-packet semantic labels directly into Quinn. It does not maintain a rich state machine over multiple media classes. It does not replace the whole transport with a model-based controller."

"Instead, AMC v1 computes a utility signal from sender-visible media semantics, smooths that signal at connection scope, and lets that signal modulate ACK growth and loss backoff in a cwnd-based controller."

The semantic inputs are codec-agnostic and explicit in the core crate:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaSemantics {
    /// Codec-agnostic sender metadata for a media unit.
    ///
    /// These fields describe utility at the application boundary. AMC v1 does
    /// not imply that Quinn receives them directly per packet or per stream;
    /// the current controller consumes only a derived connection-wide runtime
    /// signal.
    pub traffic_class: TrafficClass,
    pub importance: Importance,
    pub dependency_depth: DependencyDepth,
    pub delivery_deadline: Option<Duration>,
    pub freshness_window: Option<Duration>,
    pub size_bytes: u64,
}
```

Say:

"This is the central idea. The sender knows whether a unit belongs to VOD or live traffic, how important it is, whether it depends on earlier units, whether it has a delivery deadline, whether it becomes stale after a freshness window, and how large it is."

"AMC v1 then collapses those semantics into a scalar utility score."

The scorer is here:

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

        let dependency_depth_penalty = 1.0 / (1.0 + f64::from(inputs.semantics.dependency_depth.0));
        let dependency_penalty = if inputs.dependency_ready { 1.0 } else { 0.2 };

        let deadline_penalty = inputs
            .semantics
            .delivery_deadline
            .map(|deadline| expiry_penalty(deadline, inputs.queue_delay + inputs.estimated_rtt))
            .unwrap_or(1.0);

        let freshness_penalty = inputs
            .semantics
            .freshness_window
            .map(|window| expiry_penalty(window, inputs.queue_delay))
            .unwrap_or(1.0);

        let size_penalty = (inputs.semantics.size_bytes.max(1) as f64).sqrt();

        UtilityScore(
            (importance_weight
                * traffic_weight
                * dependency_depth_penalty
                * dependency_penalty
                * deadline_penalty
                * freshness_penalty)
                / size_penalty,
        )
    }
}
```

Say:

"There are several important design choices in that formula. Critical units get more weight than normal ones. Live units get more weight than VOD units. Deeper dependencies are penalized. Units whose dependencies are not ready are heavily penalized. Units near expiry are penalized. Larger units are penalized by the square root of size."

"So the scorer is not trying to maximize raw bytes. It is trying to prioritize useful and timely media delivery."

"After that, the score is converted into a transport control signal."

```rust
impl UtilitySignal {
    pub fn from_score_for_traffic_class(traffic_class: TrafficClass, score: UtilityScore) -> Self {
        let normalized = match traffic_class {
            TrafficClass::Vod => (score.0 * 96.0).clamp(0.0, 1.0).sqrt(),
            TrafficClass::Live => (score.0 * 128.0).clamp(0.0, 1.0).sqrt(),
        };

        let (ack_gain, loss_reduction_factor) = match traffic_class {
            TrafficClass::Vod => (0.55 + (0.35 * normalized), 0.50 + (0.15 * normalized)),
            TrafficClass::Live => (1.0 + (1.0 * normalized), 0.72 + (0.16 * normalized)),
        };

        Self {
            traffic_class,
            score,
            ack_gain,
            loss_reduction_factor,
        }
    }
}
```

Say:

"This is where the behavior difference becomes concrete. Live traffic gets a more aggressive ACK gain range than VOD. Live traffic also keeps a larger fraction of its window after a loss than VOD. In other words, AMC explicitly says that live traffic is allowed to be more aggressive than VOD for the same normalized utility score."

"That is one of the strongest code-level reasons the live results improve while VOD startup remains weaker."

The v1 scope boundary is also explicit in the runtime state comment:

```rust
impl RuntimeUtilityState {
    /// Shared bridge from sender logic into the congestion controller.
    ///
    /// AMC v1 stores only the latest connection-wide utility sample. The
    /// controller does not see per-stream state, per-packet annotations, or a
    /// history of utility changes. That boundary is deliberate for the current
    /// experiment path and is the main extension point for AMC v2.
    pub fn new() -> Self {
        Self::default()
    }
}
```

Say:

"This comment is one of the most important honesty lines in the repository. AMC v1 is connection-wide and lossy in what it retains. It does not know everything about every packet. It only sees the latest smoothed utility sample."

---

## Section 4: How AMC Changes The Controller

Say:

"AMC still uses a congestion window. It is not replacing Quinn with a fully different transport model. The control law stays cwnd-based. The difference is that utility changes how much the window grows on acknowledgments and how far it backs off on loss."

The controller initialization and scope are described here:

```rust
impl AmcController {
    /// Builds the AMC v1 controller.
    ///
    /// The control law remains connection-wide and cwnd-based. Utility affects
    /// only ACK growth and loss backoff through the latest `RuntimeUtilityState`
    /// snapshot rather than through packet-granular Quinn hooks.
    fn new(
        runtime_state: Arc<RuntimeUtilityState>,
        now: Instant,
        current_mtu: u64,
        initial_window_datagrams: u64,
        min_window_datagrams: u64,
        max_window_datagrams: u64,
    ) -> Self {
        let initial_window = initial_window_datagrams * current_mtu;

        let controller = Self {
            runtime_state,
            current_mtu,
            initial_window,
            min_window_datagrams,
            max_window_datagrams,
            window: initial_window,
            ssthresh: u64::MAX,
            recovery_start_time: now,
        };
        controller.publish_controller_snapshot(AmcControllerEvent::Initialized, None);
        controller
    }
}
```

The key ACK-path behavior is here:

```rust
fn on_end_acks(
    &mut self,
    _now: Instant,
    _in_flight: u64,
    app_limited: bool,
    largest_packet_num_acked: Option<u64>,
) {
    if app_limited || largest_packet_num_acked.is_none() {
        return;
    }

    let signal = self.runtime_state.snapshot();
    let growth = self.growth_step(signal);
    self.window =
        (self.window + growth).clamp(self.min_window(), self.class_max_window(signal));
    self.publish_controller_snapshot(AmcControllerEvent::Ack, None);
}
```

The loss-path behavior is here:

```rust
fn on_congestion_event(
    &mut self,
    now: Instant,
    sent: Instant,
    is_persistent_congestion: bool,
    _lost_bytes: u64,
) {
    if sent <= self.recovery_start_time {
        return;
    }

    self.recovery_start_time = now;
    let signal = self.runtime_state.snapshot();
    self.window = ((self.window as f64) * signal.loss_reduction_factor).round() as u64;
    self.window = self.window.max(self.min_window());
    self.window = self.window.min(self.class_max_window(signal));
    self.ssthresh = self.window;

    if is_persistent_congestion {
        self.window = self.min_window();
    }

    self.publish_controller_snapshot(
        if is_persistent_congestion {
            AmcControllerEvent::PersistentCongestion
        } else {
            AmcControllerEvent::Loss
        },
        Some(AmcControllerPhase::Recovery),
    );
}
```

And the class-specific cap and growth calculation are here:

```rust
fn class_max_window(&self, signal: UtilitySignal) -> u64 {
    match signal.traffic_class {
        TrafficClass::Vod => (self.max_window() / 2).max(self.min_window()),
        TrafficClass::Live => self.max_window(),
    }
}

fn growth_step(&self, signal: UtilitySignal) -> u64 {
    let base_gain = match signal.traffic_class {
        TrafficClass::Vod => signal.ack_gain * 0.75,
        TrafficClass::Live => signal.ack_gain,
    };

    if self.window < self.ssthresh {
        ((self.current_mtu as f64 * base_gain).round() as u64).max(self.current_mtu / 4)
    } else {
        let additive =
            ((self.current_mtu * self.current_mtu) as f64 / self.window as f64) * base_gain;
        (additive.round() as u64).max(self.current_mtu / 16)
    }
}
```

Say:

"These code excerpts explain the observed behavior better than any abstract claim. AMC grows faster when the current semantic signal says the traffic is urgent and valuable. AMC backs off less harshly when the current signal stays strong. Live traffic gets a higher cap than VOD, and VOD gets a deliberately conservative cap at half of the controller's max window."

"So the design intent is very clear: protect and prioritize timely live delivery more than buffered VOD delivery."

---

## Section 5: How The Sender Produces The Runtime Utility

Say:

"The sender computes the runtime utility at send time, using replay timing, payload size, RTT estimates from the current QUIC connection, and the semantic profile derived from the workload."

The sender-side update path is here:

```rust
fn update_runtime_utility(
    connection: &quinn::Connection,
    runtime_utility: &RuntimeUtilityState,
    mode: ReplayMode,
    start_time_ms: u64,
    duration_ms: u64,
    deadline_ms: u64,
    client_send_elapsed_ms: u64,
    payload_len: u64,
    profile: RuntimeUtilityProfile,
) -> RuntimeUtilityTelemetry {
    let traffic_class = match mode {
        ReplayMode::Vod => TrafficClass::Vod,
        ReplayMode::Live => TrafficClass::Live,
    };
    let queue_delay_ms = client_send_elapsed_ms.saturating_sub(start_time_ms);
    let estimated_rtt = connection.rtt();
    let mut semantics = MediaSemantics::new(traffic_class, profile.importance, payload_len)
        .with_dependency_depth(profile.dependency_depth);

    let deadline_budget_ms = deadline_ms.saturating_sub(start_time_ms).max(duration_ms);
    semantics = semantics.with_delivery_deadline(Duration::from_millis(deadline_budget_ms));

    let freshness_window_ms = profile.freshness_window_ms.unwrap_or(match mode {
        ReplayMode::Vod => deadline_budget_ms,
        ReplayMode::Live => duration_ms.max(1),
    });
    let dependency_ready = derive_dependency_ready(profile, queue_delay_ms, freshness_window_ms);
    semantics = semantics.with_freshness_window(Duration::from_millis(freshness_window_ms));

    let inputs = UtilityInputs {
        semantics: semantics.clone(),
        queue_delay: Duration::from_millis(queue_delay_ms),
        estimated_rtt,
        dependency_ready,
    };
    let observed_signal = UtilitySignal::from_score_for_traffic_class(
        traffic_class,
        DefaultUtilityScorer.score(&inputs),
    );
    let signal = runtime_utility.update_from_inputs(&DefaultUtilityScorer, &inputs);

    RuntimeUtilityTelemetry {
        traffic_class,
        importance: profile.importance,
        dependency_depth: profile.dependency_depth,
        dependency_ready,
        queue_delay_ms,
        estimated_rtt_ms: estimated_rtt.as_millis() as u64,
        utility_score: signal.score.0,
        observed_utility_score: Some(observed_signal.score.0),
        smoothed_utility_score: Some(signal.score.0),
        ewma_weight: Some(UTILITY_SIGNAL_EWMA_WEIGHT),
        ack_gain: signal.ack_gain,
        loss_reduction_factor: signal.loss_reduction_factor,
        controller_snapshot: runtime_utility
            .controller_snapshot()
            .map(amc_controller_snapshot_telemetry),
    }
}
```

Say:

"There are two details to emphasize here. First, utility depends on queue delay and RTT, so urgency is evaluated relative to transport conditions, not in isolation. Second, the sender smooths the signal with EWMA before the controller uses it. That avoids violent instantaneous oscillation, but it also means AMC v1 reacts with some inertia."

"That smoothing is one reason AMC can improve over loss-based baselines without matching BBR's latency behavior. It makes AMC more stable, but it also limits how sharply it can pivot."

---

## Section 6: The Experimental Architecture

Say:

"This repository is structured as a Cargo workspace with four main crates. The amc-core crate contains semantic and policy logic. The demo-client crate generates replay traffic and selects controllers. The demo-server crate acts as the receiver and report sink. The harness crate orchestrates suites, applies network shaping, computes metrics, plots results, packages reports, and drives the live demo."

At the workspace level, Quinn is pinned here:

```toml
[workspace.dependencies]
quinn = { version = "0.11.9", default-features = false, features = ["log", "platform-verifier", "ring", "runtime-tokio", "rustls-ring"] }
```

Say:

"That matters because the controller comparison is anchored to a specific Quinn version and its current built-in congestion implementations."

---

## Section 7: The Canonical Evidence Path

Say:

"The final evidence in this repository comes from exactly two canonical VPS suites. One suite is the fixed preset controller matrix. The other is the host-run coexistence fairness guardrail against BBR. The methodology is intentionally split because the Docker runner still cannot emit coexistence raw reports for multiple foreground clients."

The canonical fixed matrix config begins like this:

```json
{
  "suite_name": "vps_fixed_preset_controller_matrix",
  "host": "demo-server",
  "base_port": 5001,
  "cert_path": "results/vps/demo-cert.der",
  "replay_manifest": "data/processed/manifests/sintel_trailer_replay.json",
  "server_startup_delay_ms": 500,
  "results_root": "results/vps"
}
```

Say:

"Let me explain each top-level setting. The suite name labels every derived artifact. The host tells the client how to reach the server. The base port is the starting UDP port for runs in the suite. The cert path is the Quinn certificate for TLS validation. The replay manifest points to preprocessed media timing and semantic hints. The server startup delay gives the receiver time to come up before the client begins. The results root determines where raw and processed outputs are written."

The network presets are explicit and fixed:

```json
{
  "name": "lte_constrained",
  "kind": "linux_tc_netem",
  "description": "Constrained LTE-style preset with higher latency, stronger jitter, and lower bandwidth headroom.",
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
  },
  "notes": "Worst fixed preset in the main benchmark family before adding separate fairness studies."
}
```

Say:

"This is a fixed preset, not a dynamic trace. RTT is 110 milliseconds. Random loss is 1.5 percent. Bandwidth is limited to 8 megabits per second. Additional jitter is injected, queue depth is bounded with the netem packet limit, and token bucket filtering constrains rate with burst and latency settings."

"This preset is intentionally hard. It is where the repository expects semantic awareness to matter most, because freshness-sensitive traffic is most exposed under constrained bandwidth, higher delay, and loss."

The semantic profile is also explicit:

```json
"semantic_profile": {
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

Say:

"This is the semantic policy for the workloads. The first three startup segments are treated as critical. After startup, VOD steady-state segments are normal importance, while live steady-state segments are high importance. Every fourth segment is treated as independent by default, and dependent segments get depth one. VOD gets a thirty-second freshness window, which is effectively permissive. Live gets a one-second freshness window, which is much tighter."

"That combination is exactly why AMC is oriented toward live media rather than buffered playback."

The run matrix is explicit as well:

```json
{
  "name": "live_realtime_amc_preview_lte_constrained",
  "controller": "amc_preview",
  "mode": "live",
  "pace": "realtime",
  "network_scenario": "lte_constrained",
  "vod_deadline_slack_ms": 30000
}
```

Say:

"Each run chooses one controller, one workload mode, one pacing mode, and one fixed network preset. Pace set to realtime means the sender follows media timing rather than dumping as fast as possible. The VOD deadline slack field exists so VOD deadlines are explicit and comparable across controllers."

---

## Section 8: What The Fairness Guardrail Adds

Say:

"The second canonical suite is the fairness guardrail. It runs a foreground controller against a BBR competitor in the same live mode and the same network preset, then measures throughput share and Jain fairness."

The coexistence config makes that explicit:

```json
{
  "name": "live_realtime_amc_preview_lte_constrained_with_bbr",
  "controller": "amc_preview",
  "mode": "live",
  "pace": "realtime",
  "network_scenario": "lte_constrained",
  "vod_deadline_slack_ms": 30000,
  "coexistence": {
    "controller": "bbr",
    "mode": "live",
    "pace": "realtime",
    "vod_deadline_slack_ms": 30000
  }
}
```

Say:

"The purpose is not to show that AMC beats BBR in head-to-head live quality under competition. The purpose is narrower. It checks that AMC does not obtain its improvements by taking an unfair share of the bottleneck."

The fairness metric implementation is very direct:

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
        competitor_throughput_share: competitor_throughput / total,
        throughput_ratio: if competitor_throughput <= f64::EPSILON {
            0.0
        } else {
            foreground_throughput / competitor_throughput
        },
        jain_fairness_index: if sum_sq <= f64::EPSILON {
            0.0
        } else {
            (sum * sum) / (2.0 * sum_sq)
        },
    }
}
```

Say:

"So fairness here means throughput sharing fairness, not a broader claim about equal application quality. That distinction is important, and the repository documents it clearly."

---

## Section 9: How The Harness Validates And Applies The Setup

Say:

"The harness is doing more than launching binaries. It validates that the experiment matrix is well-formed, that every network scenario exists, that there are no duplicate matrix cells, that freshness windows are nonzero, and that coexistence settings are aligned with the foreground run."

The validation function is long, but its intent is clear from the top-level struct and the validation rules:

```rust
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
}
```

Say:

"The value of this validation layer is reproducibility. The benchmark surface is not left to ad hoc shell commands or manual interpretation. The experiment matrix is explicit, validated, and serializable."

The Linux shaping path is also explicit:

```rust
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
```

Say:

"This means the path impairment is not an abstract label. It is concretely implemented with Linux tc netem and token bucket filtering. That is one reason the methodology argues for repeatability rather than broad realism claims."

---

## Section 10: How The Harness Computes The Main Metrics

Say:

"The harness computes media-aware metrics from the raw transfer report. It reconstructs semantics from the replay manifest or harness defaults, scores units, tracks usefulness, deadline misses, delivery latency, age of information, jitter, and for VOD it reconstructs startup delay and rebuffer behavior."

The analysis path begins like this:

```rust
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
    let mut delivery_latencies_ms = Vec::new();
    let mut age_of_information_ms = Vec::new();
    let mut last_delivery_latency_ms = None;
    let mut jitter_sum = 0u64;
```

Say:

"The two most important media-facing live metrics are useful-media ratio and deadline-miss rate. Useful-media ratio tells us how much of the media received on time is still meaningful. Deadline-miss rate tells us how often media arrives too late to matter. Average age of information and jitter provide the latency-freshness picture behind those outcomes."

"For VOD, startup delay is particularly important because a buffered workload can tolerate latency after playback begins more easily than it can tolerate slow startup."

---

## Section 11: Results You Can State Directly

### 11.1 Live, WiFi Unstable, Solo

Say:

"The cleanest bounded success case for AMC v1 is the WiFi unstable live cell. Here AMC reaches a useful-media ratio of 1.0 and a deadline-miss rate of 0.0, which matches BBR and beats NewReno's useful-media ratio of 0.976 and deadline-miss rate of 0.024. Cubic also reaches 1.0 useful ratio and zero misses in this cell, but BBR still clearly leads on latency and jitter."

"Specifically, in WiFi unstable live, AMC's average delivery latency is about 140 milliseconds and average jitter is about 112 milliseconds. BBR cuts that to about 54 milliseconds latency and 47 milliseconds jitter. So AMC is good enough to preserve usefulness here, but not as strong as BBR at keeping the path tight."

Grounding values from the processed comparison export:

| controller | useful media ratio | deadline miss rate | avg latency ms | avg jitter ms | avg aoi ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| amc_preview | 1.0000 | 0.0000 | 140.14 | 111.78 | 141.10 |
| bbr | 1.0000 | 0.0000 | 53.64 | 47.49 | 54.00 |
| cubic | 1.0000 | 0.0000 | 85.24 | 96.80 | 85.86 |
| new_reno | 0.9762 | 0.0238 | 176.36 | 181.22 | 176.67 |

### 11.2 Live, LTE Constrained, Solo

Say:

"The hardest live cell is LTE constrained, and this is where the main bounded claim comes from. AMC improves over both loss-based baselines. AMC reaches a useful-media ratio of about 0.976 with a deadline-miss rate of about 0.024. Cubic falls to about 0.833 useful ratio with a miss rate of about 0.167. NewReno reaches about 0.881 useful ratio with a miss rate of about 0.119."

"But BBR still wins decisively. BBR reaches a useful-media ratio of 1.0, zero deadline misses, about 34 milliseconds average delivery latency, about 43 milliseconds jitter, and about 34 milliseconds average age of information. AMC is much better than Cubic and NewReno here, but it is still materially behind BBR on freshness-sensitive latency behavior."

Grounding values from the processed comparison export:

| controller | useful media ratio | deadline miss rate | avg latency ms | avg jitter ms | avg aoi ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| amc_preview | 0.9762 | 0.0238 | 242.76 | 227.88 | 243.64 |
| bbr | 1.0000 | 0.0000 | 33.79 | 42.51 | 34.07 |
| cubic | 0.8333 | 0.1667 | 341.05 | 240.61 | 341.24 |
| new_reno | 0.8810 | 0.1190 | 448.81 | 336.02 | 449.26 |

### 11.3 VOD, Why AMC Is Not A Startup Winner

Say:

"The VOD story is honest and limited. All controllers preserve continuity under the constrained presets. Useful-media ratio stays at 1.0 and rebuffer ratio stays at 0.0. But AMC is slower to start. In LTE constrained VOD, AMC's startup delay is 2412 milliseconds, while BBR starts at 2018 milliseconds. In WiFi unstable VOD, AMC starts at 2160 milliseconds, while BBR starts at 1239 milliseconds."

"That result fits the code. VOD uses a lower traffic weight, lower ACK aggressiveness, stronger loss backoff, and a class max window capped at half of the configured maximum. Those are intentional choices in AMC v1. They help keep the claim focused on live media utility, but they also limit startup competitiveness for buffered playback."

Grounding values from the processed comparison export:

| scenario | controller | startup delay ms | useful media ratio | rebuffer ratio |
| --- | --- | ---: | ---: | ---: |
| lte_constrained | amc_preview | 2412 | 1.0 | 0.0 |
| lte_constrained | bbr | 2018 | 1.0 | 0.0 |
| lte_constrained | cubic | 2282 | 1.0 | 0.0 |
| lte_constrained | new_reno | 2416 | 1.0 | 0.0 |
| wifi_unstable | amc_preview | 2160 | 1.0 | 0.0 |
| wifi_unstable | bbr | 1239 | 1.0 | 0.0 |
| wifi_unstable | cubic | 2109 | 1.0 | 0.0 |
| wifi_unstable | new_reno | 2120 | 1.0 | 0.0 |

### 11.4 Fairness Against BBR

Say:

"The fairness guardrail shows that AMC does not get its live improvements by taking an unfair throughput share from BBR. In LTE constrained coexistence, AMC's foreground throughput share is about 0.5002 and Jain fairness is effectively 1.0. In WiFi unstable coexistence, AMC's throughput share is about 0.4998 and Jain fairness is again effectively 1.0."

"So the right claim is that AMC preserves fair bottleneck sharing against BBR at the throughput level. The guardrail does not mean AMC matches BBR's live quality when both are competing. It means AMC is not cheating on fairness."

Grounding values from the fairness export:

| scenario | controller | fg share | throughput ratio | Jain fairness |
| --- | --- | ---: | ---: | ---: |
| lte_constrained with bbr | amc_preview | 0.500232 | 1.000928 | 0.9999998 |
| wifi_unstable with bbr | amc_preview | 0.499842 | 0.999366 | 0.9999999 |

---

## Section 12: Why The Results Look The Way They Do

Say:

"Now I want to justify the observed results from the implementation, not just repeat the numbers."

"First, AMC beats NewReno and Cubic in the hardest constrained live cells because it is explicitly biased toward timely live usefulness. Live traffic gets a higher traffic weight, a tighter freshness interpretation, a higher ACK gain range, and a less punitive loss reduction factor. Units that are expired, dependency-blocked, or likely stale are scored lower. That means the sender and controller emphasize what is still valuable right now, rather than treating all bytes as equally good."

"Second, AMC still trails BBR because the controller remains connection-wide and cwnd-based. AMC v1 is smarter than generic loss-based control, but it is still reacting through window growth and loss backoff, and it only sees a smoothed scalar utility signal. BBR, by contrast, is built around path modeling and queue-sensitive operating behavior. That gives BBR a structural advantage on latency, age of information, and jitter."

"Third, AMC is not a VOD startup winner because the code intentionally makes VOD more conservative. VOD gets a lower traffic weight than live, reduced ACK gain, stronger backoff on loss, and a class max window capped at half of the global max. Those choices make sense if the design goal is to prioritize live freshness over buffered playback startup, but they predict exactly the VOD startup result we see."

"Fourth, fairness remains acceptable because AMC is not bypassing the transport's core controls. It still lives inside Quinn's congestion-controller interface, still obeys a congestion window, and still backs off under loss. So its semantic adaptation improves prioritization without obviously grabbing a disproportionate throughput share."

---

## Section 13: A Good Word-For-Word Walkthrough

### Slide 1: Title

Say:

"This talk is about quinn-amc, a Rust research workspace that explores whether sender-visible multimedia semantics can improve QUIC outcomes over standard congestion-control baselines."

### Slide 2: Narrow Claim

Say:

"The repository's claim is intentionally narrow. AMC v1 improves the hardest constrained live cells relative to NewReno and Cubic. BBR remains the strongest overall live baseline. VOD continuity is stable, but AMC is not a startup-delay winner. Fairness against BBR is acceptable at the throughput-sharing level."

### Slide 3: Why This Matters

Say:

"Traditional congestion control usually optimizes transport health without knowing which application data matters most. Multimedia traffic is different because some units are urgent, some are stale quickly, some depend on earlier units, and some are much more valuable than others."

### Slide 4: Baselines

Say:

"The baseline controllers are Quinn's built-in NewReno, Cubic, and BBR implementations. NewReno and Cubic are loss-based window controllers. BBR is model-based and generally much stronger on latency-sensitive metrics."

### Slide 5: AMC v1 Idea

Say:

"AMC v1 does not replace QUIC with a fully semantic transport. Instead, it computes a utility score from sender-visible media semantics and uses that score to adjust congestion-window growth and loss backoff."

### Slide 6: Semantic Inputs

Say:

"The sender provides codec-agnostic metadata: traffic class, importance, dependency depth, delivery deadline, freshness window, and size. That is enough to tell the controller which data is urgent and which data is no longer worth prioritizing."

### Slide 7: Utility Formula

Say:

"The scorer increases weight for critical and live traffic, penalizes deep or blocked dependencies, penalizes units near expiry, and penalizes larger units. So the score favors small, urgent, useful media units."

### Slide 8: Controller Mapping

Say:

"AMC maps that score into an ACK gain and a loss reduction factor. Live traffic is intentionally more aggressive than VOD for the same score. That is visible directly in the code."

### Slide 9: Why v1 Is Limited

Say:

"AMC v1 only stores the latest smoothed connection-wide utility sample. It does not use per-packet annotations or a richer multi-state semantic model. That is why the repository stops at a bounded AMC v1 claim."

### Slide 10: Experiment Matrix

Say:

"The evaluation is preset-driven. Workloads are VOD and live. Controllers are NewReno, Cubic, BBR, and AMC preview. Network presets are wired clean, WiFi moderate, WiFi unstable, LTE moderate, and LTE constrained."

### Slide 11: Why Fixed Presets

Say:

"The project favors repeatability over broad realism claims. The fixed presets use Linux tc netem and token bucket shaping with explicit RTT, jitter, loss, and bandwidth parameters. That makes comparisons reproducible."

### Slide 12: Semantic Profile

Say:

"The semantic profile marks startup segments as critical, treats live steady-state traffic as high importance, and gives live a one-second freshness window while VOD gets a thirty-second window. This is a live-first policy by design."

### Slide 13: Metrics

Say:

"For live traffic, the primary metrics are useful-media ratio, deadline-miss rate, age of information, delivery latency, and jitter. For VOD, the primary metrics are startup delay and rebuffer behavior. Fairness is measured separately with throughput share and Jain fairness."

### Slide 14: WiFi Unstable Live

Say:

"In WiFi unstable live, AMC reaches the same perfect useful-media ratio and zero deadline misses as BBR, while beating NewReno. But BBR still has much lower latency and jitter, so AMC is useful here without matching BBR's freshness performance."

### Slide 15: LTE Constrained Live

Say:

"In LTE constrained live, AMC is clearly better than Cubic and NewReno on useful-media ratio and deadline misses. That is the strongest bounded success case. But BBR still wins overall on latency, jitter, and age of information."

### Slide 16: VOD Story

Say:

"For VOD, all controllers preserve continuity in the constrained presets, but AMC is slower to start. That is consistent with the code because VOD is intentionally treated more conservatively than live."

### Slide 17: Fairness Story

Say:

"Against BBR in coexistence, AMC stays near a fifty-fifty throughput share and near-perfect Jain fairness. So AMC's bounded gains are not coming from obvious unfairness."

### Slide 18: Final Takeaway

Say:

"The right conclusion is not that AMC beats BBR. The right conclusion is that a small amount of sender-visible semantic information can improve the hardest constrained live cases relative to generic loss-based baselines, while preserving acceptable throughput fairness, and without overstating what AMC v1 can do."

---

## Section 14: Short Answers To Likely Questions

### If someone asks, why not claim parity with BBR?

Say:

"Because the evidence does not support that claim. BBR remains stronger on live latency, jitter, and age of information across the canonical matrix."

### If someone asks, why does AMC help live but not VOD startup?

Say:

"Because the code explicitly gives live traffic more aggressive growth and a larger cap, while VOD is intentionally more conservative."

### If someone asks, is AMC learning or predicting the path?

Say:

"No. AMC v1 is not a learning-based controller and not a full model-based controller. It is a semantics-aware modulation of a cwnd-based QUIC controller."

### If someone asks, what would AMC v2 likely add?

Say:

"The clearest extension would be richer controller state than a single latest utility sample, potentially including more history, more traffic classes, or finer-grained transport hooks."

---

## Section 15: Canonical Reproduction Commands

Say:

"If you want to reproduce the canonical evidence path, these are the commands documented by the repository."

```bash
cd /home/leven/quinn-amc
sudo bash scripts/experiments/run_linux_vps_suite.sh configs/harness/vps_fixed_preset_controller_matrix.json
source "$HOME/.cargo/env"
cargo build -p harness
sudo ./target/debug/harness run-suite --config configs/harness/vps_host_live_coexistence_bbr_guardrail.json
sudo chown -R "$USER":"$USER" results/vps
```

```powershell
cargo run -p harness -- plot-suite --comparison results/vps/processed/harness/vps_fixed_preset_controller_matrix_comparison.json --output-dir results/vps/figures/harness
cargo run -p harness -- plot-suite --comparison results/vps/processed/harness/vps_host_live_coexistence_bbr_guardrail_comparison.json --output-dir results/vps/figures/harness
cargo run -p harness -- package-report --report docs/final-report.md --matrix-comparison results/vps/processed/harness/vps_fixed_preset_controller_matrix_comparison.json --fairness-comparison results/vps/processed/harness/vps_host_live_coexistence_bbr_guardrail_comparison.json --figure-dir results/vps/figures/harness --output-dir results/vps/reports/final
cargo run -p harness -- live-demo --report results/vps/raw/harness/live_realtime_amc_preview_lte_constrained_report.json --speed 1.0
```

---

## Closing Script

Say:

"To close, this repository shows that semantics can matter, but they matter within a very specific boundary. AMC v1 improves live performance over generic loss-based baselines in the hardest constrained presets because it knows which media is urgent and still useful. At the same time, BBR remains stronger overall because its model-based transport behavior is better at controlling latency and preserving freshness."

"So the honest result is not a universal win. It is a credible and reproducible step: semantic awareness can improve the hardest live cases without breaking fairness, and the next generation of work would need richer state and stronger transport integration to close the remaining gap to BBR."
