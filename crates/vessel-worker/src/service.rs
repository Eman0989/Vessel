use std::{
    collections::BTreeMap,
    sync::{Mutex, MutexGuard},
    time::Duration,
};

use vessel_core::{
    ArtifactRef, Node, NodeId, NodeStatus, ResourceCapacity, ResourceRequest, WorkerHeartbeat,
    WorkerRegistration,
};
use vessel_runtime::WasmRuntime;

use crate::{
    ArtifactCache, ArtifactCacheError, ExecutionRequest, ExecutionResult, WorkerConfig, WorkerError,
};

pub struct WorkerService {
    node: Mutex<Node>,
    endpoint: String,
    runtime: WasmRuntime,
    artifacts: ArtifactCache,
}

impl WorkerService {
    pub fn new(config: WorkerConfig) -> Self {
        Self::with_runtime_and_registry(config, WasmRuntime::new(), "http://127.0.0.1:7002")
    }

    pub fn with_registry(config: WorkerConfig, registry_url: impl Into<String>) -> Self {
        Self::with_runtime_and_registry(config, WasmRuntime::new(), registry_url)
    }

    pub fn with_registry_and_timeouts(
        config: WorkerConfig,
        registry_url: impl Into<String>,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self, ArtifactCacheError> {
        let artifacts =
            ArtifactCache::with_timeouts(registry_url, connect_timeout, request_timeout)?;

        Ok(Self::with_runtime_and_artifact_cache(
            config,
            WasmRuntime::new(),
            artifacts,
        ))
    }

    pub fn with_runtime(config: WorkerConfig, runtime: WasmRuntime) -> Self {
        Self::with_runtime_and_registry(config, runtime, "http://127.0.0.1:7002")
    }

    pub fn with_runtime_and_registry(
        config: WorkerConfig,
        runtime: WasmRuntime,
        registry_url: impl Into<String>,
    ) -> Self {
        Self::with_runtime_and_artifact_cache(config, runtime, ArtifactCache::new(registry_url))
    }

    fn with_runtime_and_artifact_cache(
        config: WorkerConfig,
        runtime: WasmRuntime,
        artifacts: ArtifactCache,
    ) -> Self {
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
            endpoint: config.endpoint,
            runtime,
            artifacts,
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

    pub fn registration(&self) -> Result<WorkerRegistration, WorkerError> {
        Ok(WorkerRegistration::new(
            self.node_snapshot()?,
            self.endpoint.clone(),
        ))
    }

    pub fn heartbeat(&self) -> Result<WorkerHeartbeat, WorkerError> {
        let node = self.node_snapshot()?;

        Ok(WorkerHeartbeat::from_node(&node))
    }

    pub async fn artifact(&self, artifact: &ArtifactRef) -> Result<Vec<u8>, WorkerError> {
        Ok(self.artifacts.fetch(artifact).await?)
    }

    pub fn artifact_cache(&self) -> &ArtifactCache {
        &self.artifacts
    }

    pub fn drain(&self) -> Result<(), WorkerError> {
        self.lock_node()?.status = NodeStatus::Draining;
        Ok(())
    }

    pub fn resume(&self) -> Result<(), WorkerError> {
        self.lock_node()?.status = NodeStatus::Ready;
        Ok(())
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
