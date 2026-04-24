use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrafficClass {
    Vod,
    Live,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Importance {
    Background,
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DependencyDepth(pub u8);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaSemantics {
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
