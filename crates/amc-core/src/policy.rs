use std::time::Duration;

use crate::semantics::{Importance, MediaSemantics, TrafficClass};

#[derive(Debug, Clone, PartialEq)]
pub struct UtilityInputs {
    pub semantics: MediaSemantics,
    pub queue_delay: Duration,
    pub estimated_rtt: Duration,
    pub dependency_ready: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct UtilityScore(pub f64);

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
    use std::time::Duration;

    use super::{DefaultUtilityScorer, UtilityInputs, UtilityScorer};
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
}
