use std::{
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    extract::{Extension, Path, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use vessel_core::{
    AutoscalingDecision, AutoscalingPolicy, Deployment, DeploymentId, ExecutionRequest,
    ExecutionResult, Instance, InstanceId, InstanceStatus, Node, NodeId, NodeStatus,
    WorkerHeartbeat, WorkerRegistration, Workload, WorkloadId,
};
use vessel_telemetry::ClusterMetrics;

use crate::{ControlError, ControlState};

pub type SharedState = Arc<Mutex<ControlState>>;
type ApiError = (StatusCode, Json<ErrorResponse>);

const DEFAULT_GATEWAY_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const DEFAULT_GATEWAY_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlNetworkConfig {
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
}

impl ControlNetworkConfig {
    pub const fn new(connect_timeout: Duration, request_timeout: Duration) -> Self {
        Self {
            connect_timeout,
            request_timeout,
        }
    }
}

impl Default for ControlNetworkConfig {
    fn default() -> Self {
        Self::new(
            DEFAULT_GATEWAY_CONNECT_TIMEOUT,
            DEFAULT_GATEWAY_REQUEST_TIMEOUT,
        )
    }
}

#[derive(Clone)]
struct GatewayClient {
    client: reqwest::Client,
}

impl GatewayClient {
    fn new(config: ControlNetworkConfig) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(config.connect_timeout)
            .timeout(config.request_timeout)
            .build()
            .expect("gateway HTTP client configuration must be valid");

        Self { client }
    }
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

#[derive(Debug, Serialize, Deserialize)]
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
pub struct EnableAutoscalingRequest {
    pub min_replicas: u32,
    pub max_replicas: u32,
    pub target_cpu_utilization_percent: u8,
}

#[derive(Debug, Deserialize)]
pub struct EvaluateAutoscalingRequest {
    pub observed_cpu_utilization_percent: u8,
}

#[derive(Debug, Deserialize)]
pub struct RolloutDeploymentRequest {
    pub workload_id: WorkloadId,
}

#[derive(Debug, Deserialize)]
pub struct CanaryDeploymentRequest {
    pub workload_id: WorkloadId,
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
    router_with_network_config(state, ControlNetworkConfig::default())
}

pub fn router_with_network_config(state: ControlState, network: ControlNetworkConfig) -> Router {
    shared_router_with_network_config(Arc::new(Mutex::new(state)), network)
}

pub fn shared_router(state: SharedState) -> Router {
    shared_router_with_network_config(state, ControlNetworkConfig::default())
}

pub fn shared_router_with_network_config(
    state: SharedState,
    network: ControlNetworkConfig,
) -> Router {
    let gateway = GatewayClient::new(network);

    Router::new()
        .route("/health", get(health))
        .route("/v1/metrics", get(cluster_metrics))
        .route("/v1/cluster/register", post(register_worker))
        .route("/v1/cluster/heartbeat", post(record_heartbeat))
        .route("/v1/nodes", get(list_nodes).post(register_node))
        .route("/v1/nodes/{id}/status", post(update_node_status))
        .route("/v1/workloads", get(list_workloads).post(register_workload))
        .route(
            "/v1/deployments",
            get(list_deployments).post(create_deployment),
        )
        .route("/v1/deployments/{id}/scale", post(scale_deployment))
        .route(
            "/v1/deployments/{id}/autoscaling",
            post(enable_deployment_autoscaling),
        )
        .route(
            "/v1/deployments/{id}/autoscaling/disable",
            post(disable_deployment_autoscaling),
        )
        .route(
            "/v1/deployments/{id}/autoscaling/evaluate",
            post(evaluate_deployment_autoscaling),
        )
        .route("/v1/deployments/{id}/rollout", post(rollout_deployment))
        .route("/v1/deployments/{id}/canary", post(begin_canary_deployment))
        .route(
            "/v1/deployments/{id}/promote",
            post(promote_canary_deployment),
        )
        .route("/v1/deployments/{id}/rollback", post(rollback_deployment))
        .route("/v1/deployments/{id}/reconcile", post(reconcile_deployment))
        .route("/v1/instances", get(list_instances).post(create_instance))
        .route("/v1/instances/{id}/assign", post(assign_instance))
        .route("/v1/instances/{id}/schedule", post(schedule_instance))
        .route("/v1/instances/{id}/invoke", post(invoke_instance))
        .route("/v1/instances/{id}/transition", post(transition_instance))
        .layer(Extension(gateway))
        .with_state(state)
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
        | ControlError::InstanceAlreadyExists(_)
        | ControlError::CanaryAlreadyActive(_)
        | ControlError::CanaryRequiresHealthyDeployment { .. }
        | ControlError::CanaryNotActive(_)
        | ControlError::CanaryNotReady { .. }
        | ControlError::RollbackUnavailable(_)
        | ControlError::AutoscalingControlsReplicas(_)
        | ControlError::AutoscalingNotEnabled(_) => StatusCode::CONFLICT,

        ControlError::NodeNotFound(_)
        | ControlError::WorkloadNotFound(_)
        | ControlError::DeploymentNotFound(_)
        | ControlError::InstanceNotFound(_) => StatusCode::NOT_FOUND,

        ControlError::InstanceWorkloadMismatch { .. }
        | ControlError::InstanceAssignmentRequiresNode(_)
        | ControlError::InvalidDeploymentInitialState(_)
        | ControlError::AutoscalingReplicaBounds { .. }
        | ControlError::AutoscalingPolicy(_)
        | ControlError::AutoscalingDecision(_)
        | ControlError::CanaryPlan(_)
        | ControlError::Core(_) => StatusCode::UNPROCESSABLE_ENTITY,

        ControlError::Scheduler(_) => StatusCode::SERVICE_UNAVAILABLE,
    };

    (
        status,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}

fn observed_at_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

async fn register_worker(
    State(state): State<SharedState>,
    Json(registration): Json<WorkerRegistration>,
) -> Result<Json<Node>, ApiError> {
    let node = lock_state(&state)?.register_worker(registration, observed_at_ms());

    Ok(Json(node))
}

async fn record_heartbeat(
    State(state): State<SharedState>,
    Json(heartbeat): Json<WorkerHeartbeat>,
) -> Result<Json<Node>, ApiError> {
    let node = lock_state(&state)?
        .record_heartbeat(heartbeat, observed_at_ms())
        .map_err(control_error_response)?;

    Ok(Json(node))
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn cluster_metrics(
    State(state): State<SharedState>,
) -> Result<Json<ClusterMetrics>, ApiError> {
    let state = lock_state(&state)?;

    Ok(Json(state.metrics()))
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

async fn enable_deployment_autoscaling(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(request): Json<EnableAutoscalingRequest>,
) -> Result<Json<Deployment>, ApiError> {
    let policy = AutoscalingPolicy {
        min_replicas: request.min_replicas,
        max_replicas: request.max_replicas,
        target_cpu_utilization_percent: request.target_cpu_utilization_percent,
    };

    let deployment = lock_state(&state)?
        .enable_deployment_autoscaling(&DeploymentId::new(id), policy)
        .map_err(control_error_response)?;

    Ok(Json(deployment))
}

async fn disable_deployment_autoscaling(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<Deployment>, ApiError> {
    let deployment = lock_state(&state)?
        .disable_deployment_autoscaling(&DeploymentId::new(id))
        .map_err(control_error_response)?;

    Ok(Json(deployment))
}

async fn evaluate_deployment_autoscaling(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(request): Json<EvaluateAutoscalingRequest>,
) -> Result<Json<AutoscalingDecision>, ApiError> {
    let decision = lock_state(&state)?
        .evaluate_deployment_autoscaling(
            &DeploymentId::new(id),
            request.observed_cpu_utilization_percent,
        )
        .map_err(control_error_response)?;

    Ok(Json(decision))
}

async fn begin_canary_deployment(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(request): Json<CanaryDeploymentRequest>,
) -> Result<Json<Deployment>, ApiError> {
    let deployment = lock_state(&state)?
        .begin_canary_deployment(
            &DeploymentId::new(id),
            &request.workload_id,
            request.replicas,
        )
        .map_err(control_error_response)?;

    Ok(Json(deployment))
}

async fn promote_canary_deployment(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<Deployment>, ApiError> {
    let deployment = lock_state(&state)?
        .promote_canary_deployment(&DeploymentId::new(id))
        .map_err(control_error_response)?;

    Ok(Json(deployment))
}

async fn rollback_deployment(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<Deployment>, ApiError> {
    let deployment = lock_state(&state)?
        .rollback_deployment(&DeploymentId::new(id))
        .map_err(control_error_response)?;

    Ok(Json(deployment))
}

async fn rollout_deployment(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(request): Json<RolloutDeploymentRequest>,
) -> Result<Json<Deployment>, ApiError> {
    let deployment = lock_state(&state)?
        .rollout_deployment(&DeploymentId::new(id), &request.workload_id)
        .map_err(control_error_response)?;

    Ok(Json(deployment))
}

async fn reconcile_deployment(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Instance>>, ApiError> {
    let instances = lock_state(&state)?
        .reconcile_deployment(&DeploymentId::new(id))
        .map_err(control_error_response)?;

    Ok(Json(instances))
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

async fn schedule_instance(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<Instance>, ApiError> {
    let instance = lock_state(&state)?
        .schedule_instance(&InstanceId::new(id))
        .map_err(control_error_response)?;

    Ok(Json(instance))
}

async fn invoke_instance(
    State(state): State<SharedState>,
    Extension(gateway): Extension<GatewayClient>,
    Path(id): Path<String>,
    Json(mut request): Json<ExecutionRequest>,
) -> Result<Json<ExecutionResult>, ApiError> {
    let instance_id = InstanceId::new(id);

    let (node_id, endpoint, resources) = {
        let state = lock_state(&state)?;

        let instance = state.instance(&instance_id).ok_or_else(|| {
            control_error_response(ControlError::InstanceNotFound(instance_id.clone()))
        })?;

        if !matches!(
            instance.status,
            InstanceStatus::Assigned | InstanceStatus::Starting | InstanceStatus::Running
        ) {
            return Err((
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: format!(
                        "instance {} is not invokable while {:?}",
                        instance.id, instance.status,
                    ),
                }),
            ));
        }

        let node_id = instance.node_id.clone().ok_or_else(|| {
            (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: format!("instance {} is not assigned to a worker", instance.id,),
                }),
            )
        })?;

        let endpoint = state
            .worker_endpoint(&node_id)
            .ok_or_else(|| {
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(ErrorResponse {
                        error: format!("worker endpoint for node {node_id} is unavailable",),
                    }),
                )
            })?
            .to_string();

        (node_id, endpoint, instance.resources)
    };

    // The placement owns the resource profile. Do not allow callers to
    // execute the instance with a different resource request.
    request.resources = resources;

    let worker_url = format!("{}/v1/execute", endpoint.trim_end_matches('/'));

    let response = gateway
        .client
        .post(worker_url)
        .json(&request)
        .send()
        .await
        .map_err(|error| {
            (
                StatusCode::BAD_GATEWAY,
                Json(ErrorResponse {
                    error: format!("failed to reach worker {node_id}: {error}",),
                }),
            )
        })?;

    let worker_status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

    if !worker_status.is_success() {
        let error = response
            .json::<ErrorResponse>()
            .await
            .map(|body| body.error)
            .unwrap_or_else(|_| format!("worker {node_id} returned HTTP {worker_status}",));

        return Err((worker_status, Json(ErrorResponse { error })));
    }

    let result = response.json::<ExecutionResult>().await.map_err(|error| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("worker {node_id} returned an invalid execution response: {error}",),
            }),
        )
    })?;

    if result.node_id != node_id {
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!(
                    "worker response node {} did not match assigned node {}",
                    result.node_id, node_id,
                ),
            }),
        ));
    }

    Ok(Json(result))
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
