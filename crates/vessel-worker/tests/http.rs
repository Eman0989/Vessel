use axum::{
    body::Body,
    http::{Request, StatusCode, header::CONTENT_TYPE},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
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
    router(WorkerService::new(WorkerConfig::new("worker-http-01")))
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
async fn status_endpoint_reports_worker_identity() {
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

    assert_eq!(json["node_id"], "worker-http-01",);
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
