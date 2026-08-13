use std::sync::{Arc, Mutex, MutexGuard};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use vessel_core::{
    Deployment, DeploymentId, Instance, InstanceId, InstanceStatus, Node, NodeId, NodeStatus,
    Workload,
};

use crate::{ControlError, ControlState};

type SharedState = Arc<Mutex<ControlState>>;
type ApiError = (StatusCode, Json<ErrorResponse>);

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Deserialize)]
pub struct NodeStatusRequest {
    pub status: NodeStatus,
}

#[derive(Debug, Deserialize)]
pub struct ScaleDeploymentRequest {
    pub replicas: u32,
}

#[derive(Debug, Deserialize)]
pub struct AssignInstanceRequest {
    pub node_id: NodeId,
}

#[derive(Debug, Deserialize)]
pub struct TransitionInstanceRequest {
    pub status: InstanceStatus,
}

pub fn router(state: ControlState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/nodes", get(list_nodes).post(register_node))
        .route("/v1/nodes/{id}/status", post(update_node_status))
        .route("/v1/workloads", get(list_workloads).post(register_workload))
        .route(
            "/v1/deployments",
            get(list_deployments).post(create_deployment),
        )
        .route("/v1/deployments/{id}/scale", post(scale_deployment))
        .route("/v1/instances", get(list_instances).post(create_instance))
        .route("/v1/instances/{id}/assign", post(assign_instance))
        .route("/v1/instances/{id}/transition", post(transition_instance))
        .with_state(Arc::new(Mutex::new(state)))
}

fn lock_state(state: &SharedState) -> Result<MutexGuard<'_, ControlState>, ApiError> {
    state.lock().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "control state lock was poisoned".to_string(),
            }),
        )
    })
}

fn control_error_response(error: ControlError) -> ApiError {
    let status = match &error {
        ControlError::NodeAlreadyExists(_)
        | ControlError::WorkloadAlreadyExists(_)
        | ControlError::DeploymentAlreadyExists(_)
        | ControlError::InstanceAlreadyExists(_) => StatusCode::CONFLICT,

        ControlError::NodeNotFound(_)
        | ControlError::WorkloadNotFound(_)
        | ControlError::DeploymentNotFound(_)
        | ControlError::InstanceNotFound(_) => StatusCode::NOT_FOUND,

        ControlError::InstanceWorkloadMismatch { .. }
        | ControlError::InstanceAssignmentRequiresNode(_)
        | ControlError::Core(_) => StatusCode::UNPROCESSABLE_ENTITY,
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

async fn list_nodes(State(state): State<SharedState>) -> Result<Json<Vec<Node>>, ApiError> {
    let state = lock_state(&state)?;

    Ok(Json(state.list_nodes()))
}

async fn register_node(
    State(state): State<SharedState>,
    Json(node): Json<Node>,
) -> Result<(StatusCode, Json<Node>), ApiError> {
    let response = node.clone();

    lock_state(&state)?
        .register_node(node)
        .map_err(control_error_response)?;

    Ok((StatusCode::CREATED, Json(response)))
}

async fn update_node_status(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(request): Json<NodeStatusRequest>,
) -> Result<Json<Node>, ApiError> {
    let node = lock_state(&state)?
        .update_node_status(&NodeId::new(id), request.status)
        .map_err(control_error_response)?;

    Ok(Json(node))
}

async fn list_workloads(State(state): State<SharedState>) -> Result<Json<Vec<Workload>>, ApiError> {
    let state = lock_state(&state)?;

    Ok(Json(state.list_workloads()))
}

async fn register_workload(
    State(state): State<SharedState>,
    Json(workload): Json<Workload>,
) -> Result<(StatusCode, Json<Workload>), ApiError> {
    let response = workload.clone();

    lock_state(&state)?
        .register_workload(workload)
        .map_err(control_error_response)?;

    Ok((StatusCode::CREATED, Json(response)))
}

async fn list_deployments(
    State(state): State<SharedState>,
) -> Result<Json<Vec<Deployment>>, ApiError> {
    let state = lock_state(&state)?;

    Ok(Json(state.list_deployments()))
}

async fn create_deployment(
    State(state): State<SharedState>,
    Json(deployment): Json<Deployment>,
) -> Result<(StatusCode, Json<Deployment>), ApiError> {
    let response = deployment.clone();

    lock_state(&state)?
        .create_deployment(deployment)
        .map_err(control_error_response)?;

    Ok((StatusCode::CREATED, Json(response)))
}

async fn scale_deployment(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(request): Json<ScaleDeploymentRequest>,
) -> Result<Json<Deployment>, ApiError> {
    let deployment = lock_state(&state)?
        .scale_deployment(&DeploymentId::new(id), request.replicas)
        .map_err(control_error_response)?;

    Ok(Json(deployment))
}

async fn list_instances(State(state): State<SharedState>) -> Result<Json<Vec<Instance>>, ApiError> {
    let state = lock_state(&state)?;

    Ok(Json(state.list_instances()))
}

async fn create_instance(
    State(state): State<SharedState>,
    Json(instance): Json<Instance>,
) -> Result<(StatusCode, Json<Instance>), ApiError> {
    let response = instance.clone();

    lock_state(&state)?
        .create_instance(instance)
        .map_err(control_error_response)?;

    Ok((StatusCode::CREATED, Json(response)))
}

async fn assign_instance(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(request): Json<AssignInstanceRequest>,
) -> Result<Json<Instance>, ApiError> {
    let instance = lock_state(&state)?
        .assign_instance(&InstanceId::new(id), &request.node_id)
        .map_err(control_error_response)?;

    Ok(Json(instance))
}

async fn transition_instance(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(request): Json<TransitionInstanceRequest>,
) -> Result<Json<Instance>, ApiError> {
    let instance = lock_state(&state)?
        .transition_instance(&InstanceId::new(id), request.status)
        .map_err(control_error_response)?;

    Ok(Json(instance))
}
