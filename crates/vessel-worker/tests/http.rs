use axum::{
    body::Body,
    http::{Request, StatusCode, header::CONTENT_TYPE},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use vessel_core::{ResourceCapacity, ResourceRequest};
use vessel_worker::{ExecutionRequest, ExecutionResult, WorkerConfig, WorkerService, router};

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

fn test_app() -> axum::Router {
    let config = WorkerConfig::new("worker-http-01")
        .with_name("http-worker")
        .with_region("test-region")
        .with_capacity(ResourceCapacity::new(2_000, 268_435_456, 4));

    router(WorkerService::new(config))
}

#[tokio::test]
async fn health_endpoint_reports_ok() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();

    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn status_endpoint_reports_worker_state_and_capacity() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/v1/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();

    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["node_id"], "worker-http-01");
    assert_eq!(json["name"], "http-worker");
    assert_eq!(json["region"], "test-region");
    assert_eq!(json["status"], "ready");

    assert_eq!(json["capacity"]["cpu_millis"], 2_000,);
    assert_eq!(json["capacity"]["memory_bytes"], 268_435_456_u64,);
    assert_eq!(json["capacity"]["max_instances"], 4,);

    assert_eq!(json["allocated"]["cpu_millis"], 0,);
    assert_eq!(json["allocated"]["memory_bytes"], 0,);
    assert_eq!(json["allocated_instances"], 0,);

    assert_eq!(json["available_capacity"], json["capacity"],);
}

#[tokio::test]
async fn execute_endpoint_runs_real_webassembly() {
    let request = ExecutionRequest::new(ADD_MODULE, "add", 20, 22);

    let body = serde_json::to_vec(&request).unwrap();

    let response = test_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/execute")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();

    let result: ExecutionResult = serde_json::from_slice(&body).unwrap();

    assert_eq!(result.node_id.as_str(), "worker-http-01",);

    assert_eq!(result.value, 42);
}

#[tokio::test]
async fn execute_endpoint_reports_runtime_failure() {
    let request = ExecutionRequest::new(ADD_MODULE, "missing-export", 20, 22);

    let body = serde_json::to_vec(&request).unwrap();

    let response = test_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/execute")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY,);
}

#[tokio::test]
async fn execute_endpoint_rejects_work_above_capacity() {
    let request = ExecutionRequest::new(ADD_MODULE, "add", 20, 22)
        .with_resources(ResourceRequest::new(2_001, 1));

    let body = serde_json::to_vec(&request).unwrap();

    let response = test_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/execute")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE,);
}

#[tokio::test]
async fn drain_endpoint_blocks_execution_until_resume() {
    let app = test_app();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/drain")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT,);

    let request = ExecutionRequest::new(ADD_MODULE, "add", 20, 22);

    let body = serde_json::to_vec(&request).unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/execute")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE,);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/resume")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT,);

    let request = ExecutionRequest::new(ADD_MODULE, "add", 20, 22);

    let body = serde_json::to_vec(&request).unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/execute")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK,);
}
