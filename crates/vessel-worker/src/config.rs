use vessel_core::{NodeId, ResourceCapacity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerConfig {
    pub node_id: NodeId,
    pub name: String,
    pub region: String,
    pub capacity: ResourceCapacity,
}

impl WorkerConfig {
    pub fn new(node_id: impl Into<NodeId>) -> Self {
        let node_id = node_id.into();

        Self {
            name: node_id.to_string(),
            node_id,
            region: "local".to_string(),
            capacity: ResourceCapacity::default(),
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = region.into();
        self
    }

    pub fn with_capacity(mut self, capacity: ResourceCapacity) -> Self {
        self.capacity = capacity;
        self
    }
}
