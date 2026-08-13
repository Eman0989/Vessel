use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde::Serialize;
use vessel_core::{NodeStatus, ResourceCapacity, ResourceRequest};

use crate::{ExecutionRequest, ExecutionResult, WorkerService};

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
        .with_state(Arc::new(worker))
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn status(State(worker): State<Arc<WorkerService>>) -> Json<WorkerStatusResponse> {
    let node = worker.node();

    Json(WorkerStatusResponse {
        node_id: node.id.to_string(),
        name: node.name.clone(),
        region: node.region.clone(),
        status: node.status,
        capacity: node.capacity,
        allocated: node.allocated,
        available_capacity: worker.available_capacity(),
        allocated_instances: node.allocated_instances,
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
