use crate::{CoreError, NodeId, ResourceCapacity, ResourceRequest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Joining,
    Ready,
    Draining,
    Unreachable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Node {
    pub id: NodeId,
    pub name: String,
    pub region: String,
    pub status: NodeStatus,
    pub capacity: ResourceCapacity,
    pub allocated: ResourceRequest,
    pub allocated_instances: u32,
    pub labels: BTreeMap<String, String>,
}

impl Node {
    pub fn available_capacity(&self) -> ResourceCapacity {
        self.capacity
            .subtract(&self.allocated, self.allocated_instances)
    }

    pub fn can_schedule(&self, request: &ResourceRequest) -> bool {
        self.status == NodeStatus::Ready && self.available_capacity().can_fit(request)
    }

    pub fn try_allocate(&mut self, request: &ResourceRequest) -> Result<(), CoreError> {
        if self.status != NodeStatus::Ready {
            return Err(CoreError::NodeNotSchedulable {
                node_id: self.id.clone(),
            });
        }

        if !self.available_capacity().can_fit(request) {
            return Err(CoreError::InsufficientCapacity {
                node_id: self.id.clone(),
            });
        }

        self.allocated.cpu_millis += request.cpu_millis;
        self.allocated.memory_bytes += request.memory_bytes;
        self.allocated_instances += 1;

        Ok(())
    }

    pub fn release(&mut self, request: &ResourceRequest) {
        self.allocated.cpu_millis = self.allocated.cpu_millis.saturating_sub(request.cpu_millis);

        self.allocated.memory_bytes = self
            .allocated
            .memory_bytes
            .saturating_sub(request.memory_bytes);

        self.allocated_instances = self.allocated_instances.saturating_sub(1);
    }
}
