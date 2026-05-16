use std::{
    any::Any,
    sync::{
        Arc,
        atomic::{AtomicU32, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use quinn::congestion::{Controller, ControllerFactory, ControllerMetrics};

use crate::semantics::{Importance, MediaSemantics, TrafficClass};

fn scale_window_for_mtu(window: u64, old_mtu: u64, new_mtu: u64) -> u64 {
    if old_mtu == 0 || new_mtu == 0 {
        return window;
    }

    window.div_ceil(old_mtu).saturating_mul(new_mtu)
}

#[derive(Debug, Clone, PartialEq)]
pub struct UtilityInputs {
    /// Sender-visible semantic inputs for one media unit before AMC v1 collapses
    /// them into a shared runtime signal.
    pub semantics: MediaSemantics,
    pub queue_delay: Duration,
    pub estimated_rtt: Duration,
    pub dependency_ready: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct UtilityScore(pub f64);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UtilitySignal {
    pub score: UtilityScore,
    pub ack_gain: f64,
    pub loss_reduction_factor: f64,
}

impl UtilitySignal {
    pub fn from_score(score: UtilityScore) -> Self {
        let normalized = (score.0 * 8.0).clamp(0.0, 1.0);

        Self {
            score,
            ack_gain: 0.5 + (1.5 * normalized),
            loss_reduction_factor: 0.35 + (0.5 * normalized),
        }
    }
}

impl Default for UtilitySignal {
    fn default() -> Self {
        Self {
            score: UtilityScore(0.0625),
            ack_gain: 1.0,
            loss_reduction_factor: 0.5,
        }
    }
}

#[derive(Debug, Default)]
pub struct RuntimeUtilityState {
    score_bits: AtomicU64,
    ack_gain_milli: AtomicU32,
    loss_reduction_milli: AtomicU32,
}

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

    pub fn update_from_inputs<S>(&self, scorer: &S, inputs: &UtilityInputs) -> UtilitySignal
    where
        S: UtilityScorer,
    {
        let signal = UtilitySignal::from_score(scorer.score(inputs));
        self.store_signal(signal);
        signal
    }

    pub fn store_signal(&self, signal: UtilitySignal) {
        self.score_bits
            .store(signal.score.0.to_bits(), Ordering::Relaxed);
        self.ack_gain_milli
            .store((signal.ack_gain * 1_000.0).round() as u32, Ordering::Relaxed);
        self.loss_reduction_milli.store(
            (signal.loss_reduction_factor * 1_000.0).round() as u32,
            Ordering::Relaxed,
        );
    }

    pub fn snapshot(&self) -> UtilitySignal {
        let score_bits = self.score_bits.load(Ordering::Relaxed);
        let ack_gain_milli = self.ack_gain_milli.load(Ordering::Relaxed);
        let loss_reduction_milli = self.loss_reduction_milli.load(Ordering::Relaxed);

        if score_bits == 0 && ack_gain_milli == 0 && loss_reduction_milli == 0 {
            return UtilitySignal::default();
        }

        UtilitySignal {
            score: UtilityScore(f64::from_bits(score_bits)),
            ack_gain: ack_gain_milli as f64 / 1_000.0,
            loss_reduction_factor: loss_reduction_milli as f64 / 1_000.0,
        }
    }
}

pub struct AmcControllerConfig {
    runtime_state: Arc<RuntimeUtilityState>,
    initial_window_datagrams: u64,
    min_window_datagrams: u64,
    max_window_datagrams: u64,
}

impl std::fmt::Debug for AmcControllerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AmcControllerConfig")
            .field("initial_window_datagrams", &self.initial_window_datagrams)
            .field("min_window_datagrams", &self.min_window_datagrams)
            .field("max_window_datagrams", &self.max_window_datagrams)
            .finish()
    }
}

impl Default for AmcControllerConfig {
    fn default() -> Self {
        Self {
            runtime_state: Arc::new(RuntimeUtilityState::default()),
            initial_window_datagrams: 10,
            min_window_datagrams: 2,
            max_window_datagrams: 200,
        }
    }
}

impl AmcControllerConfig {
    /// Injects the shared runtime signal that Quinn will sample from within the
    /// connection-wide congestion controller.
    pub fn with_runtime_state(mut self, runtime_state: Arc<RuntimeUtilityState>) -> Self {
        self.runtime_state = runtime_state;
        self
    }
}

impl ControllerFactory for AmcControllerConfig {
    fn build(self: Arc<Self>, now: Instant, current_mtu: u16) -> Box<dyn Controller> {
        Box::new(AmcController::new(
            self.runtime_state.clone(),
            now,
            current_mtu as u64,
            self.initial_window_datagrams,
            self.min_window_datagrams,
            self.max_window_datagrams,
        ))
    }
}

#[derive(Debug, Clone)]
pub struct AmcController {
    runtime_state: Arc<RuntimeUtilityState>,
    current_mtu: u64,
    initial_window: u64,
    min_window_datagrams: u64,
    max_window_datagrams: u64,
    window: u64,
    ssthresh: u64,
    recovery_start_time: Instant,
}

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

        Self {
            runtime_state,
            current_mtu,
            initial_window,
            min_window_datagrams,
            max_window_datagrams,
            window: initial_window,
            ssthresh: u64::MAX,
            recovery_start_time: now,
        }
    }

    fn min_window(&self) -> u64 {
        self.min_window_datagrams * self.current_mtu
    }

    fn max_window(&self) -> u64 {
        self.max_window_datagrams * self.current_mtu
    }

    fn growth_step(&self) -> u64 {
        let signal = self.runtime_state.snapshot();
        ((self.current_mtu as f64 * signal.ack_gain).round() as u64).max(self.current_mtu / 2)
    }
}

impl Controller for AmcController {
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

        let step = self.growth_step();
        let growth = if self.window < self.ssthresh {
            step.saturating_mul(2)
        } else {
            step
        };

        self.window = (self.window + growth).clamp(self.min_window(), self.max_window());
    }

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
        self.ssthresh = self.window;

        if is_persistent_congestion {
            self.window = self.min_window();
        }
    }

    fn on_mtu_update(&mut self, new_mtu: u16) {
        let old_mtu = self.current_mtu.max(1);
        self.current_mtu = new_mtu as u64;
        self.window = scale_window_for_mtu(self.window, old_mtu, self.current_mtu);
        self.ssthresh = scale_window_for_mtu(self.ssthresh, old_mtu, self.current_mtu);
        self.initial_window = scale_window_for_mtu(self.initial_window, old_mtu, self.current_mtu);
        self.window = self.window.clamp(self.min_window(), self.max_window());
        self.ssthresh = self.ssthresh.max(self.min_window());
    }

    fn window(&self) -> u64 {
        self.window
    }

    fn metrics(&self) -> ControllerMetrics {
        let mut metrics = ControllerMetrics::default();
        metrics.congestion_window = self.window;
        metrics.ssthresh = Some(self.ssthresh);
        metrics
    }

    fn clone_box(&self) -> Box<dyn Controller> {
        Box::new(self.clone())
    }

    fn initial_window(&self) -> u64 {
        self.initial_window
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

pub trait UtilityScorer {
    fn score(&self, inputs: &UtilityInputs) -> UtilityScore;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultUtilityScorer;

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

fn expiry_penalty(limit: Duration, observed: Duration) -> f64 {
    if observed >= limit {
        0.0
    } else {
        let remaining = (limit - observed).as_secs_f64();
        let total = limit.as_secs_f64().max(f64::EPSILON);
        remaining / total
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::{Duration, Instant}};

    use quinn::congestion::ControllerFactory;

    use super::{
        AmcControllerConfig, DefaultUtilityScorer, RuntimeUtilityState, UtilityInputs,
        UtilityScorer, UtilitySignal, UtilityScore,
    };
    use crate::semantics::{Importance, MediaSemantics, TrafficClass};

    #[test]
    fn higher_importance_scores_higher() {
        let scorer = DefaultUtilityScorer;

        let low = scorer.score(&UtilityInputs {
            semantics: MediaSemantics::new(TrafficClass::Live, Importance::Normal, 1_200),
            queue_delay: Duration::from_millis(5),
            estimated_rtt: Duration::from_millis(20),
            dependency_ready: true,
        });

        let high = scorer.score(&UtilityInputs {
            semantics: MediaSemantics::new(TrafficClass::Live, Importance::Critical, 1_200),
            queue_delay: Duration::from_millis(5),
            estimated_rtt: Duration::from_millis(20),
            dependency_ready: true,
        });

        assert!(high > low);
    }

    #[test]
    fn expired_units_score_zero() {
        let scorer = DefaultUtilityScorer;
        let score = scorer.score(&UtilityInputs {
            semantics: MediaSemantics::new(TrafficClass::Live, Importance::High, 1_200)
                .with_delivery_deadline(Duration::from_millis(25)),
            queue_delay: Duration::from_millis(10),
            estimated_rtt: Duration::from_millis(20),
            dependency_ready: true,
        });

        assert_eq!(score.0, 0.0);
    }

    #[test]
    fn deeper_dependencies_score_lower() {
        let scorer = DefaultUtilityScorer;

        let independent = scorer.score(&UtilityInputs {
            semantics: MediaSemantics::new(TrafficClass::Live, Importance::High, 1_200)
                .with_dependency_depth(0),
            queue_delay: Duration::from_millis(5),
            estimated_rtt: Duration::from_millis(20),
            dependency_ready: true,
        });

        let dependent = scorer.score(&UtilityInputs {
            semantics: MediaSemantics::new(TrafficClass::Live, Importance::High, 1_200)
                .with_dependency_depth(2),
            queue_delay: Duration::from_millis(5),
            estimated_rtt: Duration::from_millis(20),
            dependency_ready: true,
        });

        assert!(independent > dependent);
    }

    #[test]
    fn runtime_state_exposes_latest_connection_wide_signal() {
        let state = RuntimeUtilityState::default();
        let signal = UtilitySignal::from_score(UtilityScore(0.1));

        state.store_signal(signal);

        let snapshot = state.snapshot();
        assert_eq!(snapshot.score, signal.score);
        assert!((snapshot.ack_gain - signal.ack_gain).abs() < 0.001);
        assert!(
            (snapshot.loss_reduction_factor - signal.loss_reduction_factor).abs() < 0.001
        );
    }

    #[test]
    fn amc_controller_changes_window_from_runtime_signal() {
        let low_state = Arc::new(RuntimeUtilityState::default());
        low_state.store_signal(UtilitySignal::from_score(UtilityScore(0.0)));
        let high_state = Arc::new(RuntimeUtilityState::default());
        high_state.store_signal(UtilitySignal::from_score(UtilityScore(0.2)));

        let now = Instant::now();
        let low_factory = Arc::new(AmcControllerConfig::default().with_runtime_state(low_state));
        let high_factory = Arc::new(AmcControllerConfig::default().with_runtime_state(high_state));

        let mut low = low_factory.build(now, 1_200);
        let mut high = high_factory.build(now, 1_200);

        let low_before = low.window();
        let high_before = high.window();

        low.on_end_acks(now + Duration::from_millis(1), 0, false, Some(1));
        high.on_end_acks(now + Duration::from_millis(1), 0, false, Some(1));

        assert!(high.window() - high_before > low.window() - low_before);

        low.on_congestion_event(
            now + Duration::from_millis(3),
            now + Duration::from_millis(2),
            false,
            1_200,
        );
        high.on_congestion_event(
            now + Duration::from_millis(3),
            now + Duration::from_millis(2),
            false,
            1_200,
        );

        assert!(high.window() > low.window());
    }

    #[test]
    fn amc_controller_reacts_to_runtime_state_updates_after_build() {
        let runtime_state = Arc::new(RuntimeUtilityState::default());
        runtime_state.store_signal(UtilitySignal::from_score(UtilityScore(0.0)));

        let now = Instant::now();
        let factory = Arc::new(
            AmcControllerConfig::default().with_runtime_state(runtime_state.clone()),
        );
        let mut controller = factory.build(now, 1_200);

        let window_before = controller.window();
        controller.on_end_acks(now + Duration::from_millis(1), 0, false, Some(1));
        let low_growth = controller.window() - window_before;

        runtime_state.store_signal(UtilitySignal::from_score(UtilityScore(0.2)));
        let window_before_high = controller.window();
        controller.on_end_acks(now + Duration::from_millis(2), 0, false, Some(2));
        let high_growth = controller.window() - window_before_high;

        assert!(high_growth > low_growth);
    }

    #[test]
    fn mtu_updates_preserve_datagram_windows() {
        let runtime_state = Arc::new(RuntimeUtilityState::default());
        let now = Instant::now();
        let factory = Arc::new(AmcControllerConfig::default().with_runtime_state(runtime_state));
        let mut controller = factory.build(now, 1_200);

        controller.on_end_acks(now + Duration::from_millis(1), 0, false, Some(1));
        let datagrams_before = controller.window().div_ceil(1_200);

        controller.on_mtu_update(1_500);

        assert_eq!(controller.window().div_ceil(1_500), datagrams_before);
        assert_eq!(controller.initial_window().div_ceil(1_500), 10);
    }
}
