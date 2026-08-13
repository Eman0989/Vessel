use std::collections::BTreeMap;

use vessel_core::{Node, NodeId, NodeStatus, ResourceCapacity, ResourceRequest};
use vessel_runtime::WasmRuntime;

use crate::{ExecutionRequest, ExecutionResult, WorkerConfig, WorkerError};

pub struct WorkerService {
    node: Node,
    runtime: WasmRuntime,
}

impl WorkerService {
    pub fn new(config: WorkerConfig) -> Self {
        Self::with_runtime(config, WasmRuntime::new())
    }

    pub fn with_runtime(config: WorkerConfig, runtime: WasmRuntime) -> Self {
        let node = Node {
            id: config.node_id,
            name: config.name,
            region: config.region,
            status: NodeStatus::Ready,
            capacity: config.capacity,
            allocated: ResourceRequest::default(),
            allocated_instances: 0,
            labels: BTreeMap::new(),
        };

        Self { node, runtime }
    }

    pub fn node_id(&self) -> &NodeId {
        &self.node.id
    }

    pub fn node(&self) -> &Node {
        &self.node
    }

    pub fn available_capacity(&self) -> ResourceCapacity {
        self.node.available_capacity()
    }

    pub fn execute(&self, request: &ExecutionRequest) -> Result<ExecutionResult, WorkerError> {
        let value = self.runtime.invoke_i32_binary(
            &request.module_bytes,
            &request.export,
            request.lhs,
            request.rhs,
        )?;

        Ok(ExecutionResult {
            node_id: self.node.id.clone(),
            value,
        })
    }

    pub fn runtime(&self) -> &WasmRuntime {
        &self.runtime
    }
}
