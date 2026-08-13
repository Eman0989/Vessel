use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use axum::{Router, body::Bytes, extract::State, http::StatusCode, routing::get};
use tokio::net::TcpListener;
use vessel_core::ArtifactRef;
use vessel_worker::{ArtifactCacheError, WorkerConfig, WorkerError, WorkerService};

const ABC_DIGEST: &str = concat!(
    "sha256:",
    "ba7816bf8f01cfea414140de5dae2223",
    "b00361a396177a9cb410ff61f20015ad",
);

#[derive(Clone)]
struct RegistryState {
    bytes: Vec<u8>,
    requests: Arc<AtomicUsize>,
}

async fn serve_artifact(State(state): State<RegistryState>) -> Bytes {
    state.requests.fetch_add(1, Ordering::SeqCst);

    Bytes::from(state.bytes)
}

#[tokio::test]
async fn worker_fetches_and_reuses_cached_artifact() {
    let requests = Arc::new(AtomicUsize::new(0));

    let state = RegistryState {
        bytes: b"abc".to_vec(),
        requests: Arc::clone(&requests),
    };

    let app = Router::new()
        .route("/v1/artifacts/{digest}", get(serve_artifact))
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let worker = WorkerService::with_registry(
        WorkerConfig::new("artifact-worker-01"),
        format!("http://{address}"),
    );

    let artifact = ArtifactRef {
        digest: ABC_DIGEST.to_string(),
    };

    let first = worker.artifact(&artifact).await.unwrap();

    let second = worker.artifact(&artifact).await.unwrap();

    assert_eq!(first, b"abc");
    assert_eq!(second, b"abc");

    assert_eq!(requests.load(Ordering::SeqCst), 1,);

    assert!(worker.artifact_cache().contains(&artifact).unwrap());

    assert_eq!(worker.artifact_cache().len().unwrap(), 1,);

    server.abort();
}

#[tokio::test]
async fn corrupted_registry_response_is_rejected() {
    let requests = Arc::new(AtomicUsize::new(0));

    let state = RegistryState {
        bytes: b"corrupted".to_vec(),
        requests,
    };

    let app = Router::new()
        .route("/v1/artifacts/{digest}", get(serve_artifact))
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let worker = WorkerService::with_registry(
        WorkerConfig::new("artifact-worker-02"),
        format!("http://{address}"),
    );

    let artifact = ArtifactRef {
        digest: ABC_DIGEST.to_string(),
    };

    let error = worker.artifact(&artifact).await.unwrap_err();

    assert!(matches!(
        error,
        WorkerError::ArtifactCache(ArtifactCacheError::DigestMismatch { .. })
    ));

    assert!(worker.artifact_cache().is_empty().unwrap());

    server.abort();
}

#[tokio::test]
async fn malformed_digest_is_rejected_before_fetch() {
    let worker = WorkerService::with_registry(
        WorkerConfig::new("artifact-worker-03"),
        "http://127.0.0.1:1",
    );

    let artifact = ArtifactRef {
        digest: "sha256:not-a-real-digest".to_string(),
    };

    let error = worker.artifact(&artifact).await.unwrap_err();

    assert!(matches!(
        error,
        WorkerError::ArtifactCache(ArtifactCacheError::InvalidDigest { .. })
    ));
}

#[tokio::test]
async fn registry_not_found_is_surfaced() {
    let app = Router::new().route(
        "/v1/artifacts/{digest}",
        get(|| async { StatusCode::NOT_FOUND }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();

    let address = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let worker = WorkerService::with_registry(
        WorkerConfig::new("artifact-worker-04"),
        format!("http://{address}"),
    );

    let artifact = ArtifactRef {
        digest: ABC_DIGEST.to_string(),
    };

    let error = worker.artifact(&artifact).await.unwrap_err();

    assert!(matches!(
        error,
        WorkerError::ArtifactCache(ArtifactCacheError::Http(_))
    ));

    assert!(worker.artifact_cache().is_empty().unwrap());

    server.abort();
}
