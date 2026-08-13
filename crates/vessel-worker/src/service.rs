use vessel_core::NodeId;
use vessel_runtime::WasmRuntime;

use crate::{ExecutionRequest, ExecutionResult, WorkerConfig, WorkerError};

pub struct WorkerService {
    config: WorkerConfig,
    runtime: WasmRuntime,
}

impl WorkerService {
    pub fn new(config: WorkerConfig) -> Self {
        Self {
            config,
            runtime: WasmRuntime::new(),
        }
    }

    pub fn with_runtime(config: WorkerConfig, runtime: WasmRuntime) -> Self {
        Self { config, runtime }
    }

    pub fn node_id(&self) -> &NodeId {
        &self.config.node_id
    }

    pub fn execute(&self, request: &ExecutionRequest) -> Result<ExecutionResult, WorkerError> {
        let value = self.runtime.invoke_i32_binary(
            &request.module_bytes,
            &request.export,
            request.lhs,
            request.rhs,
        )?;

        Ok(ExecutionResult {
            node_id: self.config.node_id.clone(),
            value,
        })
    }

    pub fn runtime(&self) -> &WasmRuntime {
        &self.runtime
    }
}
