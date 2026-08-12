use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ResourceRequest {
    pub cpu_millis: u32,
    pub memory_bytes: u64,
}

impl ResourceRequest {
    pub const fn new(cpu_millis: u32, memory_bytes: u64) -> Self {
        Self {
            cpu_millis,
            memory_bytes,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ResourceCapacity {
    pub cpu_millis: u32,
    pub memory_bytes: u64,
    pub max_instances: u32,
}

impl ResourceCapacity {
    pub const fn new(cpu_millis: u32, memory_bytes: u64, max_instances: u32) -> Self {
        Self {
            cpu_millis,
            memory_bytes,
            max_instances,
        }
    }

    pub fn can_fit(&self, request: &ResourceRequest) -> bool {
        request.cpu_millis <= self.cpu_millis
            && request.memory_bytes <= self.memory_bytes
            && self.max_instances > 0
    }

    pub fn subtract(&self, allocated: &ResourceRequest, allocated_instances: u32) -> Self {
        Self {
            cpu_millis: self.cpu_millis.saturating_sub(allocated.cpu_millis),
            memory_bytes: self.memory_bytes.saturating_sub(allocated.memory_bytes),
            max_instances: self.max_instances.saturating_sub(allocated_instances),
        }
    }
}
