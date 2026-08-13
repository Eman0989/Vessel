use vessel_core::NodeId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerConfig {
    pub node_id: NodeId,
}

impl WorkerConfig {
    pub fn new(node_id: impl Into<NodeId>) -> Self {
        Self {
            node_id: node_id.into(),
        }
    }
}
