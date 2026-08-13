use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde::Serialize;
use vessel_core::{NodeStatus, ResourceCapacity, ResourceRequest};

use crate::{ExecutionRequest, ExecutionResult, WorkerError, WorkerService};

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

#[derive(Debug, Serialize)]
pub struct WorkerStatusResponse {
    pub node_id: String,
    pub name: String,
    pub region: String,
    pub status: NodeStatus,
    pub capacity: ResourceCapacity,
    pub allocated: ResourceRequest,
    pub available_capacity: ResourceCapacity,
    pub allocated_instances: u32,
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
        .route("/v1/drain", post(drain))
        .route("/v1/resume", post(resume))
        .with_state(Arc::new(worker))
}

fn worker_error_response(error: WorkerError) -> (StatusCode, Json<ErrorResponse>) {
    let status = match &error {
        WorkerError::Core(_) => StatusCode::SERVICE_UNAVAILABLE,
        WorkerError::Runtime(_) => StatusCode::UNPROCESSABLE_ENTITY,
        WorkerError::StatePoisoned => StatusCode::INTERNAL_SERVER_ERROR,
    };

    (
        status,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn status(
    State(worker): State<Arc<WorkerService>>,
) -> Result<Json<WorkerStatusResponse>, (StatusCode, Json<ErrorResponse>)> {
    let node = worker.node_snapshot().map_err(worker_error_response)?;

    Ok(Json(WorkerStatusResponse {
        node_id: node.id.to_string(),
        name: node.name.clone(),
        region: node.region.clone(),
        status: node.status,
        capacity: node.capacity,
        allocated: node.allocated,
        available_capacity: node.available_capacity(),
        allocated_instances: node.allocated_instances,
    }))
}

async fn drain(
    State(worker): State<Arc<WorkerService>>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    worker.drain().map_err(worker_error_response)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn resume(
    State(worker): State<Arc<WorkerService>>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    worker.resume().map_err(worker_error_response)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn execute(
    State(worker): State<Arc<WorkerService>>,
    Json(request): Json<ExecutionRequest>,
) -> Result<Json<ExecutionResult>, (StatusCode, Json<ErrorResponse>)> {
    let result = tokio::task::spawn_blocking(move || worker.execute(&request)).await;

    match result {
        Ok(Ok(result)) => Ok(Json(result)),

        Ok(Err(error)) => Err(worker_error_response(error)),

        Err(error) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        )),
    }
}
