use std::{
    any::Any,
    sync::{
        Arc,
        atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use quinn::congestion::{Controller, ControllerFactory, ControllerMetrics};

use crate::semantics::{Importance, MediaSemantics, TrafficClass};

pub const UTILITY_SIGNAL_EWMA_WEIGHT: f64 = 0.35;

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
    pub traffic_class: TrafficClass,
    pub score: UtilityScore,
    pub ack_gain: f64,
    pub loss_reduction_factor: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmcControllerPhase {
    SlowStart,
    CongestionAvoidance,
    Recovery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmcControllerEvent {
    Initialized,
    Ack,
    Loss,
    PersistentCongestion,
    MtuUpdate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmcControllerSnapshot {
    pub phase: AmcControllerPhase,
    pub last_event: AmcControllerEvent,
    pub current_mtu_bytes: u64,
    pub congestion_window_bytes: u64,
    pub ssthresh_bytes: Option<u64>,
    pub initial_window_bytes: u64,
    pub min_window_bytes: u64,
    pub max_window_bytes: u64,
    pub class_max_window_bytes: u64,
    pub growth_step_bytes: u64,
}

impl UtilitySignal {
    pub fn from_inputs(inputs: &UtilityInputs) -> Self {
        Self::from_score_for_traffic_class(
            inputs.semantics.traffic_class,
            DefaultUtilityScorer.score(inputs),
        )
    }

    pub fn from_score(score: UtilityScore) -> Self {
        Self::from_score_for_traffic_class(TrafficClass::Live, score)
    }

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

impl Default for UtilitySignal {
    fn default() -> Self {
        Self {
            traffic_class: TrafficClass::Live,
            score: UtilityScore(0.01),
            ack_gain: 1.4,
            loss_reduction_factor: 0.76,
        }
    }
}

#[derive(Debug, Default)]
pub struct RuntimeUtilityState {
    traffic_class_tag: AtomicU8,
    score_bits: AtomicU64,
    ack_gain_milli: AtomicU32,
    loss_reduction_milli: AtomicU32,
    controller_phase_tag: AtomicU8,
    controller_event_tag: AtomicU8,
    controller_current_mtu_bytes: AtomicU64,
    controller_window_bytes: AtomicU64,
    controller_ssthresh_bytes: AtomicU64,
    controller_initial_window_bytes: AtomicU64,
    controller_min_window_bytes: AtomicU64,
    controller_max_window_bytes: AtomicU64,
    controller_class_max_window_bytes: AtomicU64,
    controller_growth_step_bytes: AtomicU64,
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
        let observed = UtilitySignal::from_score_for_traffic_class(
            inputs.semantics.traffic_class,
            scorer.score(inputs),
        );
        let signal = blend_signal(self.snapshot(), observed, UTILITY_SIGNAL_EWMA_WEIGHT);
        self.store_signal(signal);
        signal
    }

    pub fn store_signal(&self, signal: UtilitySignal) {
        self.traffic_class_tag
            .store(traffic_class_tag(signal.traffic_class), Ordering::Relaxed);
        self.score_bits
            .store(signal.score.0.to_bits(), Ordering::Relaxed);
        self.ack_gain_milli.store(
            (signal.ack_gain * 1_000.0).round() as u32,
            Ordering::Relaxed,
        );
        self.loss_reduction_milli.store(
            (signal.loss_reduction_factor * 1_000.0).round() as u32,
            Ordering::Relaxed,
        );
    }

    pub fn snapshot(&self) -> UtilitySignal {
        let score_bits = self.score_bits.load(Ordering::Relaxed);
        let traffic_class_tag = self.traffic_class_tag.load(Ordering::Relaxed);
        let ack_gain_milli = self.ack_gain_milli.load(Ordering::Relaxed);
        let loss_reduction_milli = self.loss_reduction_milli.load(Ordering::Relaxed);

        if traffic_class_tag == 0
            && score_bits == 0
            && ack_gain_milli == 0
            && loss_reduction_milli == 0
        {
            return UtilitySignal::default();
        }

        UtilitySignal {
            traffic_class: traffic_class_from_tag(traffic_class_tag),
            score: UtilityScore(f64::from_bits(score_bits)),
            ack_gain: ack_gain_milli as f64 / 1_000.0,
            loss_reduction_factor: loss_reduction_milli as f64 / 1_000.0,
        }
    }

    pub fn store_controller_snapshot(&self, snapshot: AmcControllerSnapshot) {
        self.controller_phase_tag
            .store(controller_phase_tag(snapshot.phase), Ordering::Relaxed);
        self.controller_event_tag
            .store(controller_event_tag(snapshot.last_event), Ordering::Relaxed);
        self.controller_current_mtu_bytes
            .store(snapshot.current_mtu_bytes, Ordering::Relaxed);
        self.controller_window_bytes
            .store(snapshot.congestion_window_bytes, Ordering::Relaxed);
        self.controller_ssthresh_bytes.store(
            snapshot.ssthresh_bytes.unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
        self.controller_initial_window_bytes
            .store(snapshot.initial_window_bytes, Ordering::Relaxed);
        self.controller_min_window_bytes
            .store(snapshot.min_window_bytes, Ordering::Relaxed);
        self.controller_max_window_bytes
            .store(snapshot.max_window_bytes, Ordering::Relaxed);
        self.controller_class_max_window_bytes
            .store(snapshot.class_max_window_bytes, Ordering::Relaxed);
        self.controller_growth_step_bytes
            .store(snapshot.growth_step_bytes, Ordering::Relaxed);
    }

    pub fn controller_snapshot(&self) -> Option<AmcControllerSnapshot> {
        let current_mtu_bytes = self.controller_current_mtu_bytes.load(Ordering::Relaxed);
        let congestion_window_bytes = self.controller_window_bytes.load(Ordering::Relaxed);
        if current_mtu_bytes == 0 || congestion_window_bytes == 0 {
            return None;
        }

        let ssthresh_bytes = self.controller_ssthresh_bytes.load(Ordering::Relaxed);
        Some(AmcControllerSnapshot {
            phase: controller_phase_from_tag(
                self.controller_phase_tag.load(Ordering::Relaxed),
            ),
            last_event: controller_event_from_tag(
                self.controller_event_tag.load(Ordering::Relaxed),
            ),
            current_mtu_bytes,
            congestion_window_bytes,
            ssthresh_bytes: (ssthresh_bytes != u64::MAX).then_some(ssthresh_bytes),
            initial_window_bytes: self
                .controller_initial_window_bytes
                .load(Ordering::Relaxed),
            min_window_bytes: self.controller_min_window_bytes.load(Ordering::Relaxed),
            max_window_bytes: self.controller_max_window_bytes.load(Ordering::Relaxed),
            class_max_window_bytes: self
                .controller_class_max_window_bytes
                .load(Ordering::Relaxed),
            growth_step_bytes: self.controller_growth_step_bytes.load(Ordering::Relaxed),
        })
    }
}

fn blend_signal(previous: UtilitySignal, current: UtilitySignal, weight: f64) -> UtilitySignal {
    let weight = weight.clamp(0.0, 1.0);
    let blend = |previous: f64, current: f64| previous + ((current - previous) * weight);

    UtilitySignal {
        traffic_class: current.traffic_class,
        score: UtilityScore(blend(previous.score.0, current.score.0)),
        ack_gain: blend(previous.ack_gain, current.ack_gain),
        loss_reduction_factor: blend(
            previous.loss_reduction_factor,
            current.loss_reduction_factor,
        ),
    }
}

fn traffic_class_tag(traffic_class: TrafficClass) -> u8 {
    match traffic_class {
        TrafficClass::Vod => 1,
        TrafficClass::Live => 2,
    }
}

fn traffic_class_from_tag(tag: u8) -> TrafficClass {
    match tag {
        1 => TrafficClass::Vod,
        _ => TrafficClass::Live,
    }
}

fn controller_phase_tag(phase: AmcControllerPhase) -> u8 {
    match phase {
        AmcControllerPhase::SlowStart => 1,
        AmcControllerPhase::CongestionAvoidance => 2,
        AmcControllerPhase::Recovery => 3,
    }
}

fn controller_phase_from_tag(tag: u8) -> AmcControllerPhase {
    match tag {
        1 => AmcControllerPhase::SlowStart,
        3 => AmcControllerPhase::Recovery,
        _ => AmcControllerPhase::CongestionAvoidance,
    }
}

fn controller_event_tag(event: AmcControllerEvent) -> u8 {
    match event {
        AmcControllerEvent::Initialized => 1,
        AmcControllerEvent::Ack => 2,
        AmcControllerEvent::Loss => 3,
        AmcControllerEvent::PersistentCongestion => 4,
        AmcControllerEvent::MtuUpdate => 5,
    }
}

fn controller_event_from_tag(tag: u8) -> AmcControllerEvent {
    match tag {
        1 => AmcControllerEvent::Initialized,
        2 => AmcControllerEvent::Ack,
        3 => AmcControllerEvent::Loss,
        4 => AmcControllerEvent::PersistentCongestion,
        5 => AmcControllerEvent::MtuUpdate,
        _ => AmcControllerEvent::Initialized,
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
            initial_window_datagrams: 20,
            min_window_datagrams: 4,
            max_window_datagrams: 400,
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

    fn min_window(&self) -> u64 {
        self.min_window_datagrams * self.current_mtu
    }

    fn max_window(&self) -> u64 {
        self.max_window_datagrams * self.current_mtu
    }

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

    fn phase(&self) -> AmcControllerPhase {
        if self.window < self.ssthresh {
            AmcControllerPhase::SlowStart
        } else {
            AmcControllerPhase::CongestionAvoidance
        }
    }

    fn publish_controller_snapshot(
        &self,
        last_event: AmcControllerEvent,
        phase_override: Option<AmcControllerPhase>,
    ) {
        let signal = self.runtime_state.snapshot();
        self.runtime_state
            .store_controller_snapshot(AmcControllerSnapshot {
                phase: phase_override.unwrap_or_else(|| self.phase()),
                last_event,
                current_mtu_bytes: self.current_mtu,
                congestion_window_bytes: self.window,
                ssthresh_bytes: (self.ssthresh != u64::MAX).then_some(self.ssthresh),
                initial_window_bytes: self.initial_window,
                min_window_bytes: self.min_window(),
                max_window_bytes: self.max_window(),
                class_max_window_bytes: self.class_max_window(signal),
                growth_step_bytes: self.growth_step(signal),
            });
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

        let signal = self.runtime_state.snapshot();
        let growth = self.growth_step(signal);
        self.window =
            (self.window + growth).clamp(self.min_window(), self.class_max_window(signal));
        self.publish_controller_snapshot(AmcControllerEvent::Ack, None);
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

    fn on_mtu_update(&mut self, new_mtu: u16) {
        let old_mtu = self.current_mtu.max(1);
        self.current_mtu = new_mtu as u64;
        self.window = scale_window_for_mtu(self.window, old_mtu, self.current_mtu);
        self.ssthresh = scale_window_for_mtu(self.ssthresh, old_mtu, self.current_mtu);
        self.initial_window = scale_window_for_mtu(self.initial_window, old_mtu, self.current_mtu);
        self.window = self.window.clamp(self.min_window(), self.max_window());
        self.ssthresh = self.ssthresh.max(self.min_window());
        self.publish_controller_snapshot(AmcControllerEvent::MtuUpdate, None);
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
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };

    use quinn::congestion::{Controller, ControllerFactory};

    use super::{
        AmcControllerConfig, AmcControllerEvent, AmcControllerPhase, DefaultUtilityScorer,
        RuntimeUtilityState, UtilityInputs, UtilityScore, UtilityScorer, UtilitySignal,
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
        assert_eq!(snapshot.traffic_class, signal.traffic_class);
        assert_eq!(snapshot.score, signal.score);
        assert!((snapshot.ack_gain - signal.ack_gain).abs() < 0.001);
        assert!((snapshot.loss_reduction_factor - signal.loss_reduction_factor).abs() < 0.001);
    }

    #[test]
    fn live_signal_is_more_aggressive_than_vod_for_same_score() {
        let score = UtilityScore(0.01);
        let live = UtilitySignal::from_score_for_traffic_class(TrafficClass::Live, score);
        let vod = UtilitySignal::from_score_for_traffic_class(TrafficClass::Vod, score);

        assert!(live.ack_gain > vod.ack_gain);
        assert!(live.loss_reduction_factor > vod.loss_reduction_factor);
    }

    #[test]
    fn runtime_state_smooths_new_utility_observations() {
        let state = RuntimeUtilityState::default();
        let scorer = DefaultUtilityScorer;

        let low_signal = state.update_from_inputs(
            &scorer,
            &UtilityInputs {
                semantics: MediaSemantics::new(TrafficClass::Vod, Importance::Background, 16_000),
                queue_delay: Duration::from_millis(40),
                estimated_rtt: Duration::from_millis(80),
                dependency_ready: false,
            },
        );
        let high_raw = UtilitySignal::from_score(
            scorer.score(&UtilityInputs {
                semantics: MediaSemantics::new(TrafficClass::Live, Importance::Critical, 1_000)
                    .with_delivery_deadline(Duration::from_millis(120)),
                queue_delay: Duration::from_millis(2),
                estimated_rtt: Duration::from_millis(12),
                dependency_ready: true,
            }),
        );
        let high_smoothed = state.update_from_inputs(
            &scorer,
            &UtilityInputs {
                semantics: MediaSemantics::new(TrafficClass::Live, Importance::Critical, 1_000)
                    .with_delivery_deadline(Duration::from_millis(120)),
                queue_delay: Duration::from_millis(2),
                estimated_rtt: Duration::from_millis(12),
                dependency_ready: true,
            },
        );

        assert!(high_smoothed.score > low_signal.score);
        assert!(high_smoothed.score < high_raw.score);
        assert!(high_smoothed.ack_gain < high_raw.ack_gain);
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
        let factory =
            Arc::new(AmcControllerConfig::default().with_runtime_state(runtime_state.clone()));
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
    fn larger_windows_taper_growth() {
        let runtime_state = Arc::new(RuntimeUtilityState::default());
        runtime_state.store_signal(UtilitySignal::from_score(UtilityScore(0.2)));

        let now = Instant::now();
        let mut small = super::AmcController::new(runtime_state.clone(), now, 1_200, 20, 4, 400);
        let mut large = super::AmcController::new(runtime_state, now, 1_200, 20, 4, 400);
        small.window = 20 * 1_200;
        small.ssthresh = 10 * 1_200;
        large.window = 80 * 1_200;
        large.ssthresh = 10 * 1_200;

        let small_before = small.window();
        let large_before = large.window();

        small.on_end_acks(now + Duration::from_millis(1), 0, false, Some(1));
        large.on_end_acks(now + Duration::from_millis(1), 0, false, Some(1));

        assert!(small.window() - small_before > large.window() - large_before);
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
        assert_eq!(controller.initial_window().div_ceil(1_500), 20);
    }

    #[test]
    fn controller_snapshot_exposes_initial_slow_start_state() {
        let runtime_state = Arc::new(RuntimeUtilityState::default());
        let now = Instant::now();
        let factory = Arc::new(
            AmcControllerConfig::default().with_runtime_state(runtime_state.clone()),
        );
        let _controller = factory.build(now, 1_200);

        let snapshot = runtime_state.controller_snapshot().expect("controller snapshot");

        assert_eq!(snapshot.phase, AmcControllerPhase::SlowStart);
        assert_eq!(snapshot.last_event, AmcControllerEvent::Initialized);
        assert_eq!(snapshot.current_mtu_bytes, 1_200);
        assert_eq!(snapshot.congestion_window_bytes, 24_000);
        assert_eq!(snapshot.ssthresh_bytes, None);
    }

    #[test]
    fn controller_snapshot_marks_recovery_after_loss() {
        let runtime_state = Arc::new(RuntimeUtilityState::default());
        runtime_state.store_signal(UtilitySignal::from_score(UtilityScore(0.2)));

        let now = Instant::now();
        let factory = Arc::new(
            AmcControllerConfig::default().with_runtime_state(runtime_state.clone()),
        );
        let mut controller = factory.build(now, 1_200);
        controller.on_end_acks(now + Duration::from_millis(1), 0, false, Some(1));
        controller.on_congestion_event(
            now + Duration::from_millis(3),
            now + Duration::from_millis(2),
            false,
            1_200,
        );

        let snapshot = runtime_state.controller_snapshot().expect("controller snapshot");
        assert_eq!(snapshot.phase, AmcControllerPhase::Recovery);
        assert_eq!(snapshot.last_event, AmcControllerEvent::Loss);
        assert!(snapshot.ssthresh_bytes.is_some());
    }
}
