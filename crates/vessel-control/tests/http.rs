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
