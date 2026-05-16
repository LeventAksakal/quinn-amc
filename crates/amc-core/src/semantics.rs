use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TrafficClass {
    Vod,
    Live,
}

impl TrafficClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Vod => "vod",
            Self::Live => "live",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Importance {
    Background,
    Normal,
    High,
    Critical,
}

impl Importance {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct DependencyDepth(pub u8);

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

impl MediaSemantics {
    pub fn new(traffic_class: TrafficClass, importance: Importance, size_bytes: u64) -> Self {
        Self {
            traffic_class,
            importance,
            dependency_depth: DependencyDepth(0),
            delivery_deadline: None,
            freshness_window: None,
            size_bytes,
        }
    }

    pub fn with_dependency_depth(mut self, depth: u8) -> Self {
        self.dependency_depth = DependencyDepth(depth);
        self
    }

    pub fn with_delivery_deadline(mut self, deadline: Duration) -> Self {
        self.delivery_deadline = Some(deadline);
        self
    }

    pub fn with_freshness_window(mut self, freshness_window: Duration) -> Self {
        self.freshness_window = Some(freshness_window);
        self
    }
}
