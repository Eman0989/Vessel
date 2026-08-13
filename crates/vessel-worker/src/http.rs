use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde::Serialize;

use crate::{ExecutionRequest, ExecutionResult, WorkerService};

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

#[derive(Debug, Serialize)]
pub struct WorkerStatusResponse {
    pub node_id: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub fn router(worker: WorkerService) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/status", get(status))
        .route("/v1/execute", post(execute))
        .with_state(Arc::new(worker))
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn status(State(worker): State<Arc<WorkerService>>) -> Json<WorkerStatusResponse> {
    Json(WorkerStatusResponse {
        node_id: worker.node_id().to_string(),
    })
}

async fn execute(
    State(worker): State<Arc<WorkerService>>,
    Json(request): Json<ExecutionRequest>,
) -> Result<Json<ExecutionResult>, (StatusCode, Json<ErrorResponse>)> {
    let result = tokio::task::spawn_blocking(move || worker.execute(&request)).await;

    match result {
        Ok(Ok(result)) => Ok(Json(result)),

        Ok(Err(error)) => Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        )),

        Err(error) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        )),
    }
}
