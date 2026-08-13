use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use vessel_registry::{ArtifactStore, router};

fn test_app() -> axum::Router {
    router(ArtifactStore::new())
}

async fn body_bytes(response: axum::response::Response) -> axum::body::Bytes {
    response.into_body().collect().await.unwrap().to_bytes()
}

async fn body_json(response: axum::response::Response) -> Value {
    let body = body_bytes(response).await;

    serde_json::from_slice(&body).unwrap()
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

    assert_eq!(response.status(), StatusCode::OK,);

    let json = body_json(response).await;

    assert_eq!(json["status"], "ok");
}

#[tokio::test]
async fn artifact_can_be_uploaded() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/artifacts")
                .header(header::CONTENT_TYPE, "application/wasm")
                .body(Body::from("abc"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED,);

    let json = body_json(response).await;

    assert_eq!(
        json["artifact"]["digest"],
        concat!(
            "sha256:",
            "ba7816bf8f01cfea414140de5dae2223",
            "b00361a396177a9cb410ff61f20015ad",
        ),
    );

    assert_eq!(json["size_bytes"], 3,);
}

#[tokio::test]
async fn uploaded_artifact_can_be_downloaded() {
    let app = test_app();

    let upload = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/artifacts")
                .header(header::CONTENT_TYPE, "application/wasm")
                .body(Body::from("vessel-wasm"))
                .unwrap(),
        )
        .await
        .unwrap();

    let json = body_json(upload).await;

    let digest = json["artifact"]["digest"].as_str().unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/artifacts/{digest}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK,);

    assert_eq!(
        response.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/wasm",
    );

    let body = body_bytes(response).await;

    assert_eq!(body.as_ref(), b"vessel-wasm",);
}

#[tokio::test]
async fn missing_artifact_returns_not_found() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .uri("/v1/artifacts/sha256:missing")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND,);

    let json = body_json(response).await;

    assert_eq!(json["error"], "artifact sha256:missing was not found",);
}

#[tokio::test]
async fn duplicate_uploads_return_same_digest() {
    let app = test_app();

    let mut digests = Vec::new();

    for _ in 0..2 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/artifacts")
                    .header(header::CONTENT_TYPE, "application/wasm")
                    .body(Body::from("same-artifact"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED,);

        let json = body_json(response).await;

        digests.push(json["artifact"]["digest"].as_str().unwrap().to_string());
    }

    assert_eq!(digests[0], digests[1],);
}
