use std::sync::{Arc, Mutex, MutexGuard};

use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::Response,
    routing::{get, post},
};
use serde::Serialize;
use vessel_core::ArtifactRef;

use crate::{ArtifactStore, RegistryError, StoredArtifact};

type SharedStore = Arc<Mutex<ArtifactStore>>;
type ApiError = (StatusCode, Json<ErrorResponse>);

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub fn router(store: ArtifactStore) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/artifacts", post(upload_artifact))
        .route("/v1/artifacts/{digest}", get(download_artifact))
        .with_state(Arc::new(Mutex::new(store)))
}

fn lock_store(store: &SharedStore) -> Result<MutexGuard<'_, ArtifactStore>, ApiError> {
    store.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "artifact store lock was poisoned".to_string(),
            }),
        )
    })
}

fn registry_error_response(error: RegistryError) -> ApiError {
    match error {
        RegistryError::ArtifactNotFound { .. } => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
    }
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn upload_artifact(
    State(store): State<SharedStore>,
    body: Bytes,
) -> Result<(StatusCode, Json<StoredArtifact>), ApiError> {
    let stored = lock_store(&store)?.put(&body);

    Ok((StatusCode::CREATED, Json(stored)))
}

async fn download_artifact(
    State(store): State<SharedStore>,
    Path(digest): Path<String>,
) -> Result<Response, ApiError> {
    let artifact = ArtifactRef { digest };

    let bytes = {
        let store = lock_store(&store)?;

        store
            .get(&artifact)
            .map_err(registry_error_response)?
            .to_vec()
    };

    let mut response = Response::new(Body::from(bytes));

    *response.status_mut() = StatusCode::OK;

    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/wasm"),
    );

    Ok(response)
}
