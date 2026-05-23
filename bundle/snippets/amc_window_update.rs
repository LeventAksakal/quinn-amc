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