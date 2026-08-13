use std::{
    collections::BTreeMap,
    sync::{Mutex, MutexGuard},
};

use vessel_core::{Node, NodeId, NodeStatus, ResourceCapacity, ResourceRequest};
use vessel_runtime::WasmRuntime;

use crate::{ExecutionRequest, ExecutionResult, WorkerConfig, WorkerError};

pub struct WorkerService {
    node: Mutex<Node>,
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

        Self {
            node: Mutex::new(node),
            runtime,
        }
    }

    fn lock_node(&self) -> Result<MutexGuard<'_, Node>, WorkerError> {
        self.node.lock().map_err(|_| WorkerError::StatePoisoned)
    }

    pub fn node_id(&self) -> Result<NodeId, WorkerError> {
        Ok(self.lock_node()?.id.clone())
    }

    pub fn node_snapshot(&self) -> Result<Node, WorkerError> {
        Ok(self.lock_node()?.clone())
    }

    pub fn available_capacity(&self) -> Result<ResourceCapacity, WorkerError> {
        Ok(self.lock_node()?.available_capacity())
    }

    pub fn execute(&self, request: &ExecutionRequest) -> Result<ExecutionResult, WorkerError> {
        let node_id = {
            let mut node = self.lock_node()?;

            node.try_allocate(&request.resources)?;

            node.id.clone()
        };

        let execution = self.runtime.invoke_i32_binary(
            &request.module_bytes,
            &request.export,
            request.lhs,
            request.rhs,
        );

        {
            let mut node = self.lock_node()?;
            node.release(&request.resources);
        }

        let value = execution?;

        Ok(ExecutionResult { node_id, value })
    }

    pub fn runtime(&self) -> &WasmRuntime {
        &self.runtime
    }
}
