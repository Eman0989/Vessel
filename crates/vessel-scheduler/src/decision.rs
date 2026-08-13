use vessel_core::{NodeId, ResourceCapacity};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulingScore {
    pub projected_capacity: ResourceCapacity,
    pub allocated_instances: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulingDecision {
    pub node_id: NodeId,
    pub score: SchedulingScore,
}
