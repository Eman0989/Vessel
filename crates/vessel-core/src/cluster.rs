use serde::{Deserialize, Serialize};

use crate::{Node, NodeId, NodeStatus, ResourceCapacity, ResourceRequest};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkerRegistration {
    pub node: Node,
}

impl WorkerRegistration {
    pub fn new(node: Node) -> Self {
        Self { node }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkerHeartbeat {
    pub node_id: NodeId,
    pub status: NodeStatus,
    pub capacity: ResourceCapacity,
    pub allocated: ResourceRequest,
    pub allocated_instances: u32,
}

impl WorkerHeartbeat {
    pub fn from_node(node: &Node) -> Self {
        Self {
            node_id: node.id.clone(),
            status: node.status,
            capacity: node.capacity,
            allocated: node.allocated,
            allocated_instances: node.allocated_instances,
        }
    }
}
