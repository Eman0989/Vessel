use std::collections::BTreeMap;

use axum::{
    body::Body,
    http::{Request, StatusCode, header::CONTENT_TYPE},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use vessel_control::{ControlState, router};
use vessel_core::{
    ArtifactRef, Deployment, DeploymentId, DeploymentStatus, Instance, InstanceId, InstanceStatus,
    Node, NodeId, NodeStatus, ResourceCapacity, ResourceRequest, Workload, WorkloadId,
    WorkloadSpec, WorkloadStatus,
};

fn test_app() -> axum::Router {
    router(ControlState::new())
}

fn node(id: &str) -> Node {
    Node {
        id: NodeId::new(id),
        name: id.to_string(),
        region: "test-region".to_string(),
        status: NodeStatus::Ready,
        capacity: ResourceCapacity::new(4_000, 536_870_912, 8),
        allocated: ResourceRequest::default(),
        allocated_instances: 0,
        labels: BTreeMap::new(),
    }
}

fn workload(id: &str) -> Workload {
    Workload {
        id: WorkloadId::new(id),
        spec: WorkloadSpec {
            name: id.to_string(),
            artifact: ArtifactRef {
                digest: "sha256:test".to_string(),
            },
            resources: ResourceRequest::new(500, 67_108_864),
            timeout_ms: 5_000,
            environment: BTreeMap::new(),
        },
        status: WorkloadStatus::Ready,
    }
}

fn deployment(id: &str, workload_id: &str) -> Deployment {
    Deployment {
        id: DeploymentId::new(id),
        workload_id: WorkloadId::new(workload_id),
        desired_replicas: 2,
        generation: 1,
        status: DeploymentStatus::Pending,
    }
}

fn instance(id: &str, deployment_id: &str, workload_id: &str) -> Instance {
    Instance {
        id: InstanceId::new(id),
        deployment_id: DeploymentId::new(deployment_id),
        workload_id: WorkloadId::new(workload_id),
        node_id: None,
        status: InstanceStatus::Pending,
        resources: ResourceRequest::new(500, 67_108_864),
        restart_count: 0,
    }
}

async fn body_json(response: axum::response::Response) -> Value {
    let body = response.into_body().collect().await.unwrap().to_bytes();

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
async fn node_can_be_registered_and_listed() {
    let app = test_app();

    let body = serde_json::to_vec(&node("node-01")).unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/nodes")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED,);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/nodes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK,);

    let json = body_json(response).await;

    assert_eq!(json.as_array().unwrap().len(), 1);
    assert_eq!(json[0]["id"], "node-01");
}

#[tokio::test]
async fn duplicate_node_returns_conflict() {
    let app = test_app();

    for expected in [StatusCode::CREATED, StatusCode::CONFLICT] {
        let body = serde_json::to_vec(&node("node-01")).unwrap();

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/nodes")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), expected,);
    }
}

#[tokio::test]
async fn node_status_can_be_updated_over_http() {
    let app = test_app();

    let body = serde_json::to_vec(&node("node-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/nodes")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/nodes/node-01/status")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "status": "draining"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK,);

    let json = body_json(response).await;

    assert_eq!(json["status"], "draining",);
}

#[tokio::test]
async fn deployment_requires_existing_workload() {
    let app = test_app();

    let body = serde_json::to_vec(&deployment("deployment-01", "missing-workload")).unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/deployments")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND,);
}

#[tokio::test]
async fn deployment_can_be_created_and_scaled() {
    let app = test_app();

    let workload_body = serde_json::to_vec(&workload("workload-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workloads")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(workload_body))
                .unwrap(),
        )
        .await
        .unwrap();

    let deployment_body = serde_json::to_vec(&deployment("deployment-01", "workload-01")).unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/deployments")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(deployment_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED,);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/deployments/deployment-01/scale")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "replicas": 5
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK,);

    let json = body_json(response).await;

    assert_eq!(json["desired_replicas"], 5,);

    assert_eq!(json["generation"], 2,);

    assert_eq!(json["status"], "progressing",);
}

#[tokio::test]
async fn instance_can_be_assigned_and_advanced() {
    let app = test_app();

    let node_body = serde_json::to_vec(&node("node-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/nodes")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(node_body))
                .unwrap(),
        )
        .await
        .unwrap();

    let workload_body = serde_json::to_vec(&workload("workload-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workloads")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(workload_body))
                .unwrap(),
        )
        .await
        .unwrap();

    let deployment_body = serde_json::to_vec(&deployment("deployment-01", "workload-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/deployments")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(deployment_body))
                .unwrap(),
        )
        .await
        .unwrap();

    let instance_body =
        serde_json::to_vec(&instance("instance-01", "deployment-01", "workload-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/instances")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(instance_body))
                .unwrap(),
        )
        .await
        .unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/instances/instance-01/assign")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "node_id": "node-01"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK,);

    let json = body_json(response).await;

    assert_eq!(json["status"], "assigned",);

    assert_eq!(json["node_id"], "node-01",);

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/instances/instance-01/transition")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "status": "starting"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK,);

    let json = body_json(response).await;

    assert_eq!(json["status"], "starting",);
}

#[tokio::test]
async fn bare_assigned_transition_is_rejected_over_http() {
    let app = test_app();

    let workload_body = serde_json::to_vec(&workload("workload-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workloads")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(workload_body))
                .unwrap(),
        )
        .await
        .unwrap();

    let deployment_body = serde_json::to_vec(&deployment("deployment-01", "workload-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/deployments")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(deployment_body))
                .unwrap(),
        )
        .await
        .unwrap();

    let instance_body =
        serde_json::to_vec(&instance("instance-01", "deployment-01", "workload-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/instances")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(instance_body))
                .unwrap(),
        )
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/instances/instance-01/transition")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "status": "assigned"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY,);
}

#[tokio::test]
async fn worker_can_register_through_cluster_protocol() {
    use vessel_core::WorkerRegistration;

    let app = test_app();

    let registration =
        WorkerRegistration::new(node("cluster-node-01"), "http://cluster-node-01:7001");

    let body = serde_json::to_vec(&registration).unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/cluster/register")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK,);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/nodes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let json = body_json(response).await;

    assert_eq!(json.as_array().unwrap().len(), 1,);

    assert_eq!(json[0]["id"], "cluster-node-01",);
}

#[tokio::test]
async fn heartbeat_refreshes_registered_worker_over_http() {
    use vessel_core::{WorkerHeartbeat, WorkerRegistration};

    let app = test_app();

    let registration =
        WorkerRegistration::new(node("cluster-node-01"), "http://cluster-node-01:7001");

    let body = serde_json::to_vec(&registration).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/cluster/register")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let heartbeat = WorkerHeartbeat {
        node_id: NodeId::new("cluster-node-01"),
        status: NodeStatus::Draining,
        capacity: ResourceCapacity::new(8_000, 1_073_741_824, 16),
        allocated: ResourceRequest::new(500, 67_108_864),
        allocated_instances: 1,
    };

    let body = serde_json::to_vec(&heartbeat).unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/cluster/heartbeat")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK,);

    let json = body_json(response).await;

    assert_eq!(json["status"], "draining",);
    assert_eq!(json["capacity"]["cpu_millis"], 8_000,);
    assert_eq!(json["allocated_instances"], 1,);
}

#[tokio::test]
async fn heartbeat_from_unknown_worker_returns_not_found() {
    use vessel_core::WorkerHeartbeat;

    let heartbeat = WorkerHeartbeat {
        node_id: NodeId::new("missing-node"),
        status: NodeStatus::Ready,
        capacity: ResourceCapacity::default(),
        allocated: ResourceRequest::default(),
        allocated_instances: 0,
    };

    let body = serde_json::to_vec(&heartbeat).unwrap();

    let response = test_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/cluster/heartbeat")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND,);
}

#[tokio::test]
async fn pending_instance_can_be_scheduled_over_http() {
    let app = test_app();

    for node in [
        {
            let mut node = node("node-low");
            node.allocated = ResourceRequest::new(3_000, 268_435_456);
            node.allocated_instances = 4;
            node
        },
        node("node-high"),
    ] {
        let body = serde_json::to_vec(&node).unwrap();

        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/nodes")
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    let body = serde_json::to_vec(&workload("workload-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workloads")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = serde_json::to_vec(&deployment("deployment-01", "workload-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/deployments")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let body =
        serde_json::to_vec(&instance("instance-01", "deployment-01", "workload-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/instances")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/instances/instance-01/schedule")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK,);

    let json = body_json(response).await;

    assert_eq!(json["status"], "assigned",);

    assert_eq!(json["node_id"], "node-high",);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/nodes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let nodes = body_json(response).await;

    let selected = nodes
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "node-high")
        .unwrap();

    assert_eq!(selected["allocated"]["cpu_millis"], 500,);

    assert_eq!(selected["allocated_instances"], 1,);
}

#[tokio::test]
async fn scheduling_without_eligible_node_returns_service_unavailable() {
    let app = test_app();

    let mut draining = node("node-01");
    draining.status = NodeStatus::Draining;

    let body = serde_json::to_vec(&draining).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/nodes")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = serde_json::to_vec(&workload("workload-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workloads")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = serde_json::to_vec(&deployment("deployment-01", "workload-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/deployments")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let body =
        serde_json::to_vec(&instance("instance-01", "deployment-01", "workload-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/instances")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/instances/instance-01/schedule")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE,);

    let json = body_json(response).await;

    assert!(
        json["error"]
            .as_str()
            .unwrap()
            .contains("no eligible node",)
    );
}

#[tokio::test]
async fn deployment_can_be_reconciled_over_http() {
    let app = test_app();

    let body = serde_json::to_vec(&workload("workload-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workloads")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = serde_json::to_vec(&deployment("deployment-01", "workload-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/deployments")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/deployments/deployment-01/reconcile")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK,);

    let json = body_json(response).await;
    let created = json.as_array().unwrap();

    assert_eq!(created.len(), 2);

    assert_eq!(created[0]["id"], "deployment-01-replica-1",);

    assert_eq!(created[1]["id"], "deployment-01-replica-2",);

    assert_eq!(created[0]["status"], "pending",);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/instances")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let instances = body_json(response).await;

    assert_eq!(instances.as_array().unwrap().len(), 2,);
}

#[tokio::test]
async fn repeated_deployment_reconciliation_is_idempotent_over_http() {
    let app = test_app();

    let body = serde_json::to_vec(&workload("workload-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workloads")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = serde_json::to_vec(&deployment("deployment-01", "workload-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/deployments")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    for expected_count in [2, 0] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/deployments/deployment-01/reconcile")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK,);

        let json = body_json(response).await;

        assert_eq!(json.as_array().unwrap().len(), expected_count,);
    }
}

#[tokio::test]
async fn reconciling_missing_deployment_returns_not_found_over_http() {
    let response = test_app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/deployments/missing/reconcile")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND,);

    let json = body_json(response).await;

    assert!(
        json["error"]
            .as_str()
            .unwrap()
            .contains("deployment missing was not found",)
    );
}

#[tokio::test]
async fn deployment_reconciliation_schedules_replicas_when_node_is_available() {
    let app = test_app();

    let body = serde_json::to_vec(&node("node-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/nodes")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = serde_json::to_vec(&workload("workload-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/workloads")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let body = serde_json::to_vec(&deployment("deployment-01", "workload-01")).unwrap();

    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/deployments")
                .header(CONTENT_TYPE, "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/deployments/deployment-01/reconcile")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let json = body_json(response).await;
    let instances = json.as_array().unwrap();

    assert_eq!(instances.len(), 2);

    assert!(
        instances
            .iter()
            .all(|instance| instance["status"] == "assigned")
    );

    assert!(
        instances
            .iter()
            .all(|instance| instance["node_id"] == "node-01")
    );
}
