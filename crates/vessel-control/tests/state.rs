use std::collections::BTreeMap;

use vessel_control::{ControlError, ControlState};
use vessel_core::{
    ArtifactRef, Deployment, DeploymentId, DeploymentStatus, Instance, InstanceId, InstanceStatus,
    Node, NodeId, NodeStatus, ResourceCapacity, ResourceRequest, Workload, WorkloadId,
    WorkloadSpec, WorkloadStatus,
};

fn node(id: &str) -> Node {
    Node {
        id: NodeId::new(id),
        name: id.to_string(),
        region: "test".to_string(),
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

#[test]
fn control_state_registers_nodes() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();

    assert_eq!(state.node_count(), 1);

    let stored = state.node(&NodeId::new("node-01")).unwrap();

    assert_eq!(stored.name, "node-01");
}

#[test]
fn duplicate_node_is_rejected() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();

    let error = state.register_node(node("node-01")).unwrap_err();

    assert_eq!(
        error,
        ControlError::NodeAlreadyExists(NodeId::new("node-01"),),
    );
}

#[test]
fn control_state_registers_workloads() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-01")).unwrap();

    assert_eq!(state.workload_count(), 1);

    assert!(state.workload(&WorkloadId::new("workload-01")).is_some());
}

#[test]
fn duplicate_workload_is_rejected() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-01")).unwrap();

    let error = state
        .register_workload(workload("workload-01"))
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::WorkloadAlreadyExists(WorkloadId::new("workload-01"),),
    );
}

#[test]
fn deployment_requires_registered_workload() {
    let mut state = ControlState::new();

    let error = state
        .create_deployment(deployment("deployment-01", "missing-workload"))
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::WorkloadNotFound(WorkloadId::new("missing-workload"),),
    );
}

#[test]
fn deployment_is_stored_after_workload_exists() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    assert_eq!(state.deployment_count(), 1);

    assert!(
        state
            .deployment(&DeploymentId::new("deployment-01"),)
            .is_some()
    );
}

#[test]
fn instance_requires_existing_deployment() {
    let mut state = ControlState::new();

    let error = state
        .create_instance(instance("instance-01", "missing-deployment", "workload-01"))
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::DeploymentNotFound(DeploymentId::new("missing-deployment"),),
    );
}

#[test]
fn instance_workload_must_match_deployment() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-01")).unwrap();

    state.register_workload(workload("workload-02")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    let error = state
        .create_instance(instance("instance-01", "deployment-01", "workload-02"))
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::InstanceWorkloadMismatch {
            instance_id: InstanceId::new("instance-01"),
            instance_workload_id: WorkloadId::new("workload-02"),
            deployment_workload_id: WorkloadId::new("workload-01"),
        },
    );
}

#[test]
fn valid_instance_is_stored() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    state
        .create_instance(instance("instance-01", "deployment-01", "workload-01"))
        .unwrap();

    assert_eq!(state.instance_count(), 1);

    assert!(state.instance(&InstanceId::new("instance-01")).is_some());
}
