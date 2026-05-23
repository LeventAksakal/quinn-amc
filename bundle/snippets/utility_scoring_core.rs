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