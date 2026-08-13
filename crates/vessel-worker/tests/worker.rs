use vessel_worker::{ExecutionRequest, WorkerConfig, WorkerError, WorkerService};

const ADD_MODULE: &[u8] = br#"
(module
  (func (export "add")
    (param i32 i32)
    (result i32)
    local.get 0
    local.get 1
    i32.add
  )
)
"#;

#[test]
fn worker_executes_real_webassembly() {
    let worker = WorkerService::new(WorkerConfig::new("worker-01"));

    let request = ExecutionRequest::new(ADD_MODULE, "add", 20, 22);

    let result = worker.execute(&request).unwrap();

    assert_eq!(result.node_id.as_str(), "worker-01");
    assert_eq!(result.value, 42);
}

#[test]
fn worker_surfaces_runtime_execution_errors() {
    let worker = WorkerService::new(WorkerConfig::new("worker-01"));

    let request = ExecutionRequest::new(ADD_MODULE, "missing-export", 20, 22);

    let error = worker.execute(&request).unwrap_err();

    assert!(matches!(error, WorkerError::Runtime(_)));
}

#[test]
fn worker_reports_configured_node_state_and_capacity() {
    use vessel_core::{NodeStatus, ResourceCapacity};

    let capacity = ResourceCapacity::new(4_000, 536_870_912, 8);

    let config = WorkerConfig::new("worker-capacity-01")
        .with_name("worker-east-01")
        .with_region("eu-central")
        .with_capacity(capacity);

    let worker = WorkerService::new(config);

    let node = worker.node_snapshot().unwrap();

    assert_eq!(node.id.as_str(), "worker-capacity-01",);

    assert_eq!(node.name, "worker-east-01",);

    assert_eq!(node.region, "eu-central",);

    assert_eq!(node.status, NodeStatus::Ready,);

    assert_eq!(node.capacity, capacity,);

    assert_eq!(node.allocated.cpu_millis, 0,);

    assert_eq!(node.allocated.memory_bytes, 0,);

    assert_eq!(node.allocated_instances, 0,);

    assert_eq!(worker.available_capacity().unwrap(), capacity);
}

#[test]
fn worker_rejects_execution_above_capacity() {
    use vessel_core::{ResourceCapacity, ResourceRequest};

    let worker = WorkerService::new(
        WorkerConfig::new("worker-limited").with_capacity(ResourceCapacity::new(500, 1_048_576, 1)),
    );

    let request = ExecutionRequest::new(b"not even valid wasm".to_vec(), "add", 20, 22)
        .with_resources(ResourceRequest::new(600, 1_048_576));

    let error = worker.execute(&request).unwrap_err();

    assert!(matches!(error, WorkerError::Core(_)));

    let node = worker.node_snapshot().unwrap();

    assert_eq!(node.allocated_instances, 0);
}

#[test]
fn worker_releases_capacity_after_runtime_failure() {
    use vessel_core::ResourceRequest;

    let worker = WorkerService::new(WorkerConfig::new("worker-release"));

    let request = ExecutionRequest::new(ADD_MODULE, "missing-export", 20, 22)
        .with_resources(ResourceRequest::new(100, 1_024));

    let error = worker.execute(&request).unwrap_err();

    assert!(matches!(error, WorkerError::Runtime(_)));

    let node = worker.node_snapshot().unwrap();

    assert_eq!(node.allocated.cpu_millis, 0,);
    assert_eq!(node.allocated.memory_bytes, 0,);
    assert_eq!(node.allocated_instances, 0,);
}

#[test]
fn draining_worker_rejects_new_execution_until_resumed() {
    use vessel_core::{NodeStatus, ResourceRequest};

    let worker = WorkerService::new(WorkerConfig::new("worker-drain"));

    worker.drain().unwrap();

    let node = worker.node_snapshot().unwrap();

    assert_eq!(node.status, NodeStatus::Draining,);

    let request = ExecutionRequest::new(ADD_MODULE, "add", 20, 22)
        .with_resources(ResourceRequest::new(100, 1_024));

    let error = worker.execute(&request).unwrap_err();

    assert!(matches!(error, WorkerError::Core(_)));

    let node = worker.node_snapshot().unwrap();

    assert_eq!(node.allocated_instances, 0,);

    worker.resume().unwrap();

    let node = worker.node_snapshot().unwrap();

    assert_eq!(node.status, NodeStatus::Ready,);

    let result = worker.execute(&request).unwrap();

    assert_eq!(result.value, 42);
}
