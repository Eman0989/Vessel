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
