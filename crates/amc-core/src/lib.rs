//! AMC core exposes the semantic scoring inputs and the current Quinn controller hook.
//!
//! The AMC v1 boundary is intentionally narrow: the sender can score rich per-unit
//! semantics, but the congestion controller itself consumes only the latest
//! connection-wide `UtilitySignal` snapshot stored in `RuntimeUtilityState`.
//! That keeps the current controller compatible with Quinn's public congestion
//! interfaces while leaving room for later sender-state expansion.

pub mod policy;
pub mod semantics;

pub use policy::{
    AmcControllerConfig, AmcControllerEvent, AmcControllerPhase, AmcControllerSnapshot,
    DefaultUtilityScorer, RuntimeUtilityState, UtilityInputs, UtilityScore,
    UtilityScorer, UtilitySignal, UTILITY_SIGNAL_EWMA_WEIGHT,
};
pub use semantics::{DependencyDepth, Importance, MediaSemantics, TrafficClass};
