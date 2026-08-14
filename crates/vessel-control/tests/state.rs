use std::collections::BTreeMap;

use vessel_control::{ControlError, ControlState};
use vessel_core::{
    ArtifactRef, CanaryPlanError, Deployment, DeploymentId, DeploymentStatus, Instance, InstanceId,
    InstanceStatus, Node, NodeId, NodeStatus, ResourceCapacity, ResourceRequest, Workload,
    WorkloadId, WorkloadSpec, WorkloadStatus,
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
        previous_workload_id: None,
        canary: None,
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

#[test]
fn control_state_lists_resources_in_id_order() {
    let mut state = ControlState::new();

    state.register_node(node("node-02")).unwrap();
    state.register_node(node("node-01")).unwrap();

    state.register_workload(workload("workload-02")).unwrap();

    state.register_workload(workload("workload-01")).unwrap();

    let nodes = state.list_nodes();
    let workloads = state.list_workloads();

    assert_eq!(nodes.len(), 2);
    assert_eq!(workloads.len(), 2);

    assert_eq!(nodes[0].id, NodeId::new("node-01"),);

    assert_eq!(nodes[1].id, NodeId::new("node-02"),);

    assert_eq!(workloads[0].id, WorkloadId::new("workload-01"),);

    assert_eq!(workloads[1].id, WorkloadId::new("workload-02"),);
}

#[test]
fn node_status_can_be_updated() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();

    let updated = state
        .update_node_status(&NodeId::new("node-01"), NodeStatus::Draining)
        .unwrap();

    assert_eq!(updated.status, NodeStatus::Draining,);

    assert_eq!(
        state.node(&NodeId::new("node-01")).unwrap().status,
        NodeStatus::Draining,
    );
}

#[test]
fn updating_missing_node_is_rejected() {
    let mut state = ControlState::new();

    let error = state
        .update_node_status(&NodeId::new("missing-node"), NodeStatus::Draining)
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::NodeNotFound(NodeId::new("missing-node"),),
    );
}

#[test]
fn deployment_can_be_scaled() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    let updated = state
        .scale_deployment(&DeploymentId::new("deployment-01"), 5)
        .unwrap();

    assert_eq!(updated.desired_replicas, 5);
    assert_eq!(updated.generation, 2);

    assert_eq!(updated.status, DeploymentStatus::Progressing,);
}

#[test]
fn scaling_to_same_replica_count_preserves_generation() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    let updated = state
        .scale_deployment(&DeploymentId::new("deployment-01"), 2)
        .unwrap();

    assert_eq!(updated.desired_replicas, 2);
    assert_eq!(updated.generation, 1);

    assert_eq!(updated.status, DeploymentStatus::Pending,);
}

#[test]
fn deployment_can_begin_rollout_to_registered_workload() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-v1")).unwrap();
    state.register_workload(workload("workload-v2")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-v1"))
        .unwrap();

    let updated = state
        .rollout_deployment(
            &DeploymentId::new("deployment-01"),
            &WorkloadId::new("workload-v2"),
        )
        .unwrap();

    assert_eq!(updated.workload_id, WorkloadId::new("workload-v2"));

    assert_eq!(updated.generation, 2);
    assert_eq!(updated.status, DeploymentStatus::Progressing);

    let stored = state
        .deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert_eq!(stored.workload_id, WorkloadId::new("workload-v2"));
}

#[test]
fn deployment_rollout_to_same_workload_is_idempotent() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-v1")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-v1"))
        .unwrap();

    let updated = state
        .rollout_deployment(
            &DeploymentId::new("deployment-01"),
            &WorkloadId::new("workload-v1"),
        )
        .unwrap();

    assert_eq!(updated.workload_id, WorkloadId::new("workload-v1"));

    assert_eq!(updated.generation, 1);
    assert_eq!(updated.status, DeploymentStatus::Pending);
}

#[test]
fn deployment_rollout_requires_registered_workload() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-v1")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-v1"))
        .unwrap();

    let error = state
        .rollout_deployment(
            &DeploymentId::new("deployment-01"),
            &WorkloadId::new("missing-workload"),
        )
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::WorkloadNotFound(WorkloadId::new("missing-workload"))
    );

    let stored = state
        .deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert_eq!(stored.workload_id, WorkloadId::new("workload-v1"));

    assert_eq!(stored.generation, 1);
}

#[test]
fn deployment_rollout_requires_existing_deployment() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-v2")).unwrap();

    let error = state
        .rollout_deployment(
            &DeploymentId::new("missing-deployment"),
            &WorkloadId::new("workload-v2"),
        )
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::DeploymentNotFound(DeploymentId::new("missing-deployment"))
    );
}

#[test]
fn deployment_can_begin_canary_to_registered_workload() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-v1")).unwrap();
    state.register_workload(workload("workload-v2")).unwrap();

    let mut stable = deployment("deployment-01", "workload-v1");
    stable.status = DeploymentStatus::Healthy;

    state.create_deployment(stable).unwrap();

    let updated = state
        .begin_canary_deployment(
            &DeploymentId::new("deployment-01"),
            &WorkloadId::new("workload-v2"),
            1,
        )
        .unwrap();

    assert_eq!(updated.workload_id, WorkloadId::new("workload-v1"),);

    let plan = updated.canary.as_ref().unwrap();

    assert_eq!(plan.stable_workload_id, WorkloadId::new("workload-v1"),);

    assert_eq!(plan.candidate_workload_id, WorkloadId::new("workload-v2"),);

    assert_eq!(plan.candidate_replicas, 1);
    assert_eq!(updated.generation, 2);
    assert_eq!(updated.status, DeploymentStatus::Progressing);
}

#[test]
fn repeated_identical_canary_request_is_idempotent() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-v1")).unwrap();
    state.register_workload(workload("workload-v2")).unwrap();

    let mut stable = deployment("deployment-01", "workload-v1");
    stable.status = DeploymentStatus::Healthy;

    state.create_deployment(stable).unwrap();

    state
        .begin_canary_deployment(
            &DeploymentId::new("deployment-01"),
            &WorkloadId::new("workload-v2"),
            1,
        )
        .unwrap();

    let repeated = state
        .begin_canary_deployment(
            &DeploymentId::new("deployment-01"),
            &WorkloadId::new("workload-v2"),
            1,
        )
        .unwrap();

    assert_eq!(repeated.generation, 2);
    assert_eq!(repeated.status, DeploymentStatus::Progressing);
    assert!(repeated.canary.is_some());
}

#[test]
fn conflicting_canary_request_is_rejected() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-v1")).unwrap();
    state.register_workload(workload("workload-v2")).unwrap();
    state.register_workload(workload("workload-v3")).unwrap();

    let mut stable = deployment("deployment-01", "workload-v1");
    stable.status = DeploymentStatus::Healthy;

    state.create_deployment(stable).unwrap();

    state
        .begin_canary_deployment(
            &DeploymentId::new("deployment-01"),
            &WorkloadId::new("workload-v2"),
            1,
        )
        .unwrap();

    let error = state
        .begin_canary_deployment(
            &DeploymentId::new("deployment-01"),
            &WorkloadId::new("workload-v3"),
            1,
        )
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::CanaryAlreadyActive(DeploymentId::new("deployment-01"),),
    );
}

#[test]
fn canary_requires_existing_deployment() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-v2")).unwrap();

    let error = state
        .begin_canary_deployment(
            &DeploymentId::new("missing-deployment"),
            &WorkloadId::new("workload-v2"),
            1,
        )
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::DeploymentNotFound(DeploymentId::new("missing-deployment"),),
    );
}

#[test]
fn canary_requires_registered_candidate_workload() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-v1")).unwrap();

    let mut stable = deployment("deployment-01", "workload-v1");
    stable.status = DeploymentStatus::Healthy;

    state.create_deployment(stable).unwrap();

    let error = state
        .begin_canary_deployment(
            &DeploymentId::new("deployment-01"),
            &WorkloadId::new("missing-workload"),
            1,
        )
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::WorkloadNotFound(WorkloadId::new("missing-workload"),),
    );
}

#[test]
fn canary_candidate_must_differ_from_stable_workload() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-v1")).unwrap();

    let mut stable = deployment("deployment-01", "workload-v1");
    stable.status = DeploymentStatus::Healthy;

    state.create_deployment(stable).unwrap();

    let error = state
        .begin_canary_deployment(
            &DeploymentId::new("deployment-01"),
            &WorkloadId::new("workload-v1"),
            1,
        )
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::CanaryPlan(CanaryPlanError::CandidateMatchesStable,),
    );
}

#[test]
fn canary_replica_count_must_preserve_stable_capacity() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-v1")).unwrap();
    state.register_workload(workload("workload-v2")).unwrap();

    let mut stable = deployment("deployment-01", "workload-v1");
    stable.status = DeploymentStatus::Healthy;

    state.create_deployment(stable).unwrap();

    let error = state
        .begin_canary_deployment(
            &DeploymentId::new("deployment-01"),
            &WorkloadId::new("workload-v2"),
            2,
        )
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::CanaryPlan(CanaryPlanError::InvalidReplicaCount {
            desired_replicas: 2,
            candidate_replicas: 2,
        },),
    );
}

#[test]
fn canary_requires_healthy_stable_deployment() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-v1")).unwrap();
    state.register_workload(workload("workload-v2")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-v1"))
        .unwrap();

    let error = state
        .begin_canary_deployment(
            &DeploymentId::new("deployment-01"),
            &WorkloadId::new("workload-v2"),
            1,
        )
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::CanaryRequiresHealthyDeployment {
            deployment_id: DeploymentId::new("deployment-01"),
            status: DeploymentStatus::Pending,
        },
    );
}

#[test]
fn canary_reconciliation_converges_to_requested_split() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();
    state.register_workload(workload("workload-v1")).unwrap();
    state.register_workload(workload("workload-v2")).unwrap();

    state
        .create_deployment(deployment("deployment-canary", "workload-v1"))
        .unwrap();

    let initial = state
        .reconcile_deployment(&DeploymentId::new("deployment-canary"))
        .unwrap();

    assert_eq!(initial.len(), 2);

    assert_eq!(
        state
            .deployment(&DeploymentId::new("deployment-canary"),)
            .unwrap()
            .status,
        DeploymentStatus::Healthy,
    );

    state
        .begin_canary_deployment(
            &DeploymentId::new("deployment-canary"),
            &WorkloadId::new("workload-v2"),
            1,
        )
        .unwrap();

    let changed = state
        .reconcile_deployment(&DeploymentId::new("deployment-canary"))
        .unwrap();

    assert_eq!(changed.len(), 2);

    let active = state
        .list_instances()
        .into_iter()
        .filter(|instance| {
            instance.deployment_id == DeploymentId::new("deployment-canary")
                && !instance.status.is_terminal()
        })
        .collect::<Vec<_>>();

    assert_eq!(active.len(), 2);

    assert_eq!(
        active
            .iter()
            .filter(|instance| { instance.workload_id == WorkloadId::new("workload-v1") })
            .count(),
        1,
    );

    assert_eq!(
        active
            .iter()
            .filter(|instance| { instance.workload_id == WorkloadId::new("workload-v2") })
            .count(),
        1,
    );

    assert!(
        active
            .iter()
            .all(|instance| { instance.status == InstanceStatus::Assigned })
    );

    let deployment = state
        .deployment(&DeploymentId::new("deployment-canary"))
        .unwrap();

    assert_eq!(deployment.workload_id, WorkloadId::new("workload-v1"));
    assert_eq!(deployment.status, DeploymentStatus::Healthy);
    assert!(deployment.canary.is_some());

    let repeated = state
        .reconcile_deployment(&DeploymentId::new("deployment-canary"))
        .unwrap();

    assert!(repeated.is_empty());
}

#[test]
fn canary_reconciliation_waits_while_candidate_is_pending() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();
    state.register_workload(workload("workload-v1")).unwrap();

    let mut candidate = workload("workload-v2");
    candidate.spec.resources = ResourceRequest::new(5_000, 67_108_864);

    state.register_workload(candidate).unwrap();

    state
        .create_deployment(deployment("deployment-canary", "workload-v1"))
        .unwrap();

    state
        .reconcile_deployment(&DeploymentId::new("deployment-canary"))
        .unwrap();

    state
        .begin_canary_deployment(
            &DeploymentId::new("deployment-canary"),
            &WorkloadId::new("workload-v2"),
            1,
        )
        .unwrap();

    let first = state
        .reconcile_deployment(&DeploymentId::new("deployment-canary"))
        .unwrap();

    assert_eq!(first.len(), 2);

    let active = state
        .list_instances()
        .into_iter()
        .filter(|instance| {
            instance.deployment_id == DeploymentId::new("deployment-canary")
                && !instance.status.is_terminal()
        })
        .collect::<Vec<_>>();

    assert_eq!(active.len(), 2);

    assert_eq!(
        active
            .iter()
            .filter(|instance| { instance.workload_id == WorkloadId::new("workload-v1") })
            .count(),
        1,
    );

    let candidate = active
        .iter()
        .find(|instance| instance.workload_id == WorkloadId::new("workload-v2"))
        .unwrap();

    assert_eq!(candidate.status, InstanceStatus::Pending);

    assert_eq!(
        state
            .deployment(&DeploymentId::new("deployment-canary"),)
            .unwrap()
            .status,
        DeploymentStatus::Progressing,
    );

    let second = state
        .reconcile_deployment(&DeploymentId::new("deployment-canary"))
        .unwrap();

    assert!(second.is_empty());

    let active_after_second = state
        .list_instances()
        .into_iter()
        .filter(|instance| {
            instance.deployment_id == DeploymentId::new("deployment-canary")
                && !instance.status.is_terminal()
        })
        .collect::<Vec<_>>();

    assert_eq!(active_after_second.len(), 2);

    assert_eq!(
        active_after_second
            .iter()
            .filter(|instance| { instance.workload_id == WorkloadId::new("workload-v1") })
            .count(),
        1,
    );
}

#[test]
fn rollout_is_rejected_while_canary_is_active() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();
    state.register_workload(workload("workload-v1")).unwrap();
    state.register_workload(workload("workload-v2")).unwrap();
    state.register_workload(workload("workload-v3")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-v1"))
        .unwrap();

    state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    state
        .begin_canary_deployment(
            &DeploymentId::new("deployment-01"),
            &WorkloadId::new("workload-v2"),
            1,
        )
        .unwrap();

    let error = state
        .rollout_deployment(
            &DeploymentId::new("deployment-01"),
            &WorkloadId::new("workload-v3"),
        )
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::CanaryAlreadyActive(DeploymentId::new("deployment-01"),),
    );
}

#[test]
fn canary_scale_rejects_split_without_stable_capacity() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();
    state.register_workload(workload("workload-v1")).unwrap();
    state.register_workload(workload("workload-v2")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-v1"))
        .unwrap();

    state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    state
        .begin_canary_deployment(
            &DeploymentId::new("deployment-01"),
            &WorkloadId::new("workload-v2"),
            1,
        )
        .unwrap();

    let error = state
        .scale_deployment(&DeploymentId::new("deployment-01"), 1)
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::CanaryPlan(CanaryPlanError::InvalidReplicaCount {
            desired_replicas: 1,
            candidate_replicas: 1,
        },),
    );

    let deployment = state
        .deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert_eq!(deployment.desired_replicas, 2);
    assert_eq!(deployment.generation, 2);
}

#[test]
fn healthy_canary_can_be_promoted() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();
    state.register_workload(workload("workload-v1")).unwrap();
    state.register_workload(workload("workload-v2")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-v1"))
        .unwrap();

    state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    state
        .begin_canary_deployment(
            &DeploymentId::new("deployment-01"),
            &WorkloadId::new("workload-v2"),
            1,
        )
        .unwrap();

    state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    let promoted = state
        .promote_canary_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert_eq!(promoted.workload_id, WorkloadId::new("workload-v2"),);

    assert_eq!(
        promoted.previous_workload_id,
        Some(WorkloadId::new("workload-v1")),
    );

    assert_eq!(promoted.generation, 3);
    assert_eq!(promoted.status, DeploymentStatus::Progressing);
    assert!(promoted.canary.is_none());

    state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    let active = state
        .list_instances()
        .into_iter()
        .filter(|instance| {
            instance.deployment_id == DeploymentId::new("deployment-01")
                && !instance.status.is_terminal()
        })
        .collect::<Vec<_>>();

    assert_eq!(active.len(), 2);

    assert!(
        active
            .iter()
            .all(|instance| { instance.workload_id == WorkloadId::new("workload-v2") })
    );

    assert_eq!(
        state
            .deployment(&DeploymentId::new("deployment-01"))
            .unwrap()
            .status,
        DeploymentStatus::Healthy,
    );
}

#[test]
fn canary_promotion_requires_active_canary() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-v1")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-v1"))
        .unwrap();

    let error = state
        .promote_canary_deployment(&DeploymentId::new("deployment-01"))
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::CanaryNotActive(DeploymentId::new("deployment-01"),),
    );
}

#[test]
fn canary_promotion_waits_for_candidate_availability() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();
    state.register_workload(workload("workload-v1")).unwrap();

    let mut candidate = workload("workload-v2");
    candidate.spec.resources = ResourceRequest::new(5_000, 67_108_864);

    state.register_workload(candidate).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-v1"))
        .unwrap();

    state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    state
        .begin_canary_deployment(
            &DeploymentId::new("deployment-01"),
            &WorkloadId::new("workload-v2"),
            1,
        )
        .unwrap();

    state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    let error = state
        .promote_canary_deployment(&DeploymentId::new("deployment-01"))
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::CanaryNotReady {
            deployment_id: DeploymentId::new("deployment-01"),
            status: DeploymentStatus::Progressing,
        },
    );
}

#[test]
fn active_canary_can_be_rolled_back_to_stable_revision() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();
    state.register_workload(workload("workload-v1")).unwrap();
    state.register_workload(workload("workload-v2")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-v1"))
        .unwrap();

    state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    state
        .begin_canary_deployment(
            &DeploymentId::new("deployment-01"),
            &WorkloadId::new("workload-v2"),
            1,
        )
        .unwrap();

    state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    let rolled_back = state
        .rollback_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert_eq!(rolled_back.workload_id, WorkloadId::new("workload-v1"),);

    assert_eq!(rolled_back.previous_workload_id, None);
    assert_eq!(rolled_back.generation, 3);
    assert_eq!(rolled_back.status, DeploymentStatus::Progressing,);
    assert!(rolled_back.canary.is_none());

    state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    let active = state
        .list_instances()
        .into_iter()
        .filter(|instance| {
            instance.deployment_id == DeploymentId::new("deployment-01")
                && !instance.status.is_terminal()
        })
        .collect::<Vec<_>>();

    assert_eq!(active.len(), 2);

    assert!(
        active
            .iter()
            .all(|instance| { instance.workload_id == WorkloadId::new("workload-v1") })
    );

    assert_eq!(
        state
            .deployment(&DeploymentId::new("deployment-01"))
            .unwrap()
            .status,
        DeploymentStatus::Healthy,
    );
}

#[test]
fn promoted_revision_can_rollback_to_previous_workload() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();
    state.register_workload(workload("workload-v1")).unwrap();
    state.register_workload(workload("workload-v2")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-v1"))
        .unwrap();

    state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    state
        .begin_canary_deployment(
            &DeploymentId::new("deployment-01"),
            &WorkloadId::new("workload-v2"),
            1,
        )
        .unwrap();

    state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    state
        .promote_canary_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    let rolled_back = state
        .rollback_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert_eq!(rolled_back.workload_id, WorkloadId::new("workload-v1"),);

    assert_eq!(
        rolled_back.previous_workload_id,
        Some(WorkloadId::new("workload-v2")),
    );

    assert_eq!(rolled_back.generation, 4);
    assert_eq!(rolled_back.status, DeploymentStatus::Progressing,);

    state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    let active = state
        .list_instances()
        .into_iter()
        .filter(|instance| {
            instance.deployment_id == DeploymentId::new("deployment-01")
                && !instance.status.is_terminal()
        })
        .collect::<Vec<_>>();

    assert_eq!(active.len(), 2);

    assert!(
        active
            .iter()
            .all(|instance| { instance.workload_id == WorkloadId::new("workload-v1") })
    );

    assert_eq!(
        state
            .deployment(&DeploymentId::new("deployment-01"))
            .unwrap()
            .status,
        DeploymentStatus::Healthy,
    );
}

#[test]
fn rollback_requires_previous_revision_or_active_canary() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-v1")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-v1"))
        .unwrap();

    let error = state
        .rollback_deployment(&DeploymentId::new("deployment-01"))
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::RollbackUnavailable(DeploymentId::new("deployment-01"),),
    );
}

#[test]
fn instance_can_be_assigned_to_existing_node() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    state
        .create_instance(instance("instance-01", "deployment-01", "workload-01"))
        .unwrap();

    let assigned = state
        .assign_instance(&InstanceId::new("instance-01"), &NodeId::new("node-01"))
        .unwrap();

    assert_eq!(assigned.status, InstanceStatus::Assigned,);

    assert_eq!(assigned.node_id, Some(NodeId::new("node-01")),);
}

#[test]
fn assigning_instance_to_missing_node_is_rejected() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    state
        .create_instance(instance("instance-01", "deployment-01", "workload-01"))
        .unwrap();

    let error = state
        .assign_instance(
            &InstanceId::new("instance-01"),
            &NodeId::new("missing-node"),
        )
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::NodeNotFound(NodeId::new("missing-node"),),
    );
}

#[test]
fn instance_lifecycle_can_advance_after_assignment() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    state
        .create_instance(instance("instance-01", "deployment-01", "workload-01"))
        .unwrap();

    state
        .assign_instance(&InstanceId::new("instance-01"), &NodeId::new("node-01"))
        .unwrap();

    let starting = state
        .transition_instance(&InstanceId::new("instance-01"), InstanceStatus::Starting)
        .unwrap();

    assert_eq!(starting.status, InstanceStatus::Starting,);

    let running = state
        .transition_instance(&InstanceId::new("instance-01"), InstanceStatus::Running)
        .unwrap();

    assert_eq!(running.status, InstanceStatus::Running,);
}

#[test]
fn bare_assigned_transition_is_rejected() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    state
        .create_instance(instance("instance-01", "deployment-01", "workload-01"))
        .unwrap();

    let error = state
        .transition_instance(&InstanceId::new("instance-01"), InstanceStatus::Assigned)
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::InstanceAssignmentRequiresNode(InstanceId::new("instance-01"),),
    );

    let stored = state.instance(&InstanceId::new("instance-01")).unwrap();

    assert_eq!(stored.status, InstanceStatus::Pending,);

    assert_eq!(stored.node_id, None);
}

#[test]
fn invalid_instance_transition_surfaces_core_error() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    state
        .create_instance(instance("instance-01", "deployment-01", "workload-01"))
        .unwrap();

    let error = state
        .transition_instance(&InstanceId::new("instance-01"), InstanceStatus::Running)
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::Core(vessel_core::CoreError::InvalidInstanceTransition {
            from: InstanceStatus::Pending,
            to: InstanceStatus::Running,
        },),
    );
}

#[test]
fn worker_registration_is_idempotent_and_records_liveness() {
    use vessel_core::WorkerRegistration;

    let mut state = ControlState::new();

    let first = WorkerRegistration::new(node("node-cluster-01"), "http://node-cluster-01:7001");

    state.register_worker(first, 1_000);

    assert_eq!(
        state.node_last_seen_ms(&NodeId::new("node-cluster-01"),),
        Some(1_000),
    );

    assert_eq!(
        state.worker_endpoint(&NodeId::new("node-cluster-01")),
        Some("http://node-cluster-01:7001"),
    );

    let mut replacement = node("node-cluster-01");

    replacement.name = "restarted-worker".to_string();

    state.register_worker(
        WorkerRegistration::new(replacement, "http://node-cluster-01:7101"),
        2_000,
    );

    assert_eq!(state.node_count(), 1);

    assert_eq!(
        state.node(&NodeId::new("node-cluster-01")).unwrap().name,
        "restarted-worker",
    );

    assert_eq!(
        state.node_last_seen_ms(&NodeId::new("node-cluster-01"),),
        Some(2_000),
    );

    assert_eq!(
        state.worker_endpoint(&NodeId::new("node-cluster-01")),
        Some("http://node-cluster-01:7101"),
    );
}

#[test]
fn heartbeat_refreshes_node_state_and_liveness() {
    use vessel_core::{ResourceRequest, WorkerHeartbeat, WorkerRegistration};

    let mut state = ControlState::new();

    state.register_worker(
        WorkerRegistration::new(node("node-cluster-01"), "http://node-cluster-01:7001"),
        1_000,
    );

    let heartbeat = WorkerHeartbeat {
        node_id: NodeId::new("node-cluster-01"),
        status: NodeStatus::Draining,
        capacity: ResourceCapacity::new(8_000, 1_073_741_824, 16),
        allocated: ResourceRequest::new(1_000, 134_217_728),
        allocated_instances: 2,
    };

    let updated = state.record_heartbeat(heartbeat, 2_000).unwrap();

    assert_eq!(updated.status, NodeStatus::Draining,);
    assert_eq!(updated.capacity.cpu_millis, 8_000,);
    assert_eq!(updated.allocated.cpu_millis, 1_000,);
    assert_eq!(updated.allocated_instances, 2,);

    assert_eq!(
        state.node_last_seen_ms(&NodeId::new("node-cluster-01"),),
        Some(2_000),
    );
}

#[test]
fn heartbeat_from_unknown_worker_is_rejected() {
    use vessel_core::WorkerHeartbeat;

    let mut state = ControlState::new();

    let error = state
        .record_heartbeat(
            WorkerHeartbeat {
                node_id: NodeId::new("missing-node"),
                status: NodeStatus::Ready,
                capacity: ResourceCapacity::default(),
                allocated: ResourceRequest::default(),
                allocated_instances: 0,
            },
            1_000,
        )
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::NodeNotFound(NodeId::new("missing-node"),),
    );
}

#[test]
fn manual_assignment_reserves_node_capacity() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();
    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    state
        .create_instance(instance("instance-01", "deployment-01", "workload-01"))
        .unwrap();

    state
        .assign_instance(&InstanceId::new("instance-01"), &NodeId::new("node-01"))
        .unwrap();

    let node = state.node(&NodeId::new("node-01")).unwrap();

    assert_eq!(node.allocated, ResourceRequest::new(500, 67_108_864,),);

    assert_eq!(node.allocated_instances, 1,);
}

#[test]
fn failed_assignment_rolls_back_extra_reservation() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();
    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    state
        .create_instance(instance("instance-01", "deployment-01", "workload-01"))
        .unwrap();

    state
        .assign_instance(&InstanceId::new("instance-01"), &NodeId::new("node-01"))
        .unwrap();

    let error = state
        .assign_instance(&InstanceId::new("instance-01"), &NodeId::new("node-01"))
        .unwrap_err();

    assert!(matches!(
        error,
        ControlError::Core(vessel_core::CoreError::InvalidInstanceTransition {
            from: InstanceStatus::Assigned,
            to: InstanceStatus::Assigned,
        })
    ));

    let node = state.node(&NodeId::new("node-01")).unwrap();

    assert_eq!(node.allocated, ResourceRequest::new(500, 67_108_864,),);

    assert_eq!(node.allocated_instances, 1,);
}

#[test]
fn scheduler_assigns_instance_to_best_node_and_reserves_capacity() {
    let mut state = ControlState::new();

    let mut low = node("node-low");
    low.allocated = ResourceRequest::new(3_000, 268_435_456);
    low.allocated_instances = 4;

    let high = node("node-high");

    state.register_node(low).unwrap();
    state.register_node(high).unwrap();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    state
        .create_instance(instance("instance-01", "deployment-01", "workload-01"))
        .unwrap();

    let scheduled = state
        .schedule_instance(&InstanceId::new("instance-01"))
        .unwrap();

    assert_eq!(scheduled.status, InstanceStatus::Assigned,);

    assert_eq!(scheduled.node_id, Some(NodeId::new("node-high")),);

    let selected = state.node(&NodeId::new("node-high")).unwrap();

    assert_eq!(selected.allocated, ResourceRequest::new(500, 67_108_864,),);

    assert_eq!(selected.allocated_instances, 1,);
}

#[test]
fn scheduler_failure_leaves_instance_pending_and_capacity_unchanged() {
    let mut state = ControlState::new();

    let mut unavailable = node("node-01");
    unavailable.status = NodeStatus::Draining;

    state.register_node(unavailable).unwrap();
    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    state
        .create_instance(instance("instance-01", "deployment-01", "workload-01"))
        .unwrap();

    let error = state
        .schedule_instance(&InstanceId::new("instance-01"))
        .unwrap_err();

    assert!(matches!(
        error,
        ControlError::Scheduler(vessel_scheduler::SchedulerError::NoEligibleNodes { .. })
    ));

    let stored = state.instance(&InstanceId::new("instance-01")).unwrap();

    assert_eq!(stored.status, InstanceStatus::Pending,);

    assert_eq!(stored.node_id, None);

    let node = state.node(&NodeId::new("node-01")).unwrap();

    assert_eq!(node.allocated, ResourceRequest::default(),);

    assert_eq!(node.allocated_instances, 0,);
}

#[test]
fn reconciliation_creates_missing_pending_replicas() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    let created = state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert_eq!(created.len(), 2);
    assert_eq!(state.instance_count(), 2);

    assert_eq!(created[0].id, InstanceId::new("deployment-01-replica-1",),);

    assert_eq!(created[1].id, InstanceId::new("deployment-01-replica-2",),);

    for replica in &created {
        assert_eq!(replica.status, InstanceStatus::Pending,);

        assert_eq!(replica.node_id, None);

        assert_eq!(replica.workload_id, WorkloadId::new("workload-01"),);

        assert_eq!(replica.resources, ResourceRequest::new(500, 67_108_864,),);
    }

    assert_eq!(
        state
            .deployment(&DeploymentId::new("deployment-01",),)
            .unwrap()
            .status,
        DeploymentStatus::Progressing,
    );
}

#[test]
fn reconciliation_is_idempotent_when_replica_count_is_satisfied() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    let first = state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    let second = state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert_eq!(first.len(), 2);
    assert!(second.is_empty());
    assert_eq!(state.instance_count(), 2);
}

#[test]
fn terminal_replica_is_replaced_during_reconciliation() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    let created = state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    state
        .transition_instance(&created[0].id, InstanceStatus::Failed)
        .unwrap();

    let replacement = state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert_eq!(replacement.len(), 1);

    assert_eq!(
        replacement[0].id,
        InstanceId::new("deployment-01-replica-3",),
    );

    assert_eq!(replacement[0].status, InstanceStatus::Pending,);

    assert_eq!(state.instance_count(), 3);
}

#[test]
fn reconciliation_counts_only_instances_from_target_deployment() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-a", "workload-01"))
        .unwrap();

    state
        .create_deployment(deployment("deployment-b", "workload-01"))
        .unwrap();

    state
        .reconcile_deployment(&DeploymentId::new("deployment-a"))
        .unwrap();

    let created_for_b = state
        .reconcile_deployment(&DeploymentId::new("deployment-b"))
        .unwrap();

    assert_eq!(created_for_b.len(), 2);

    assert_eq!(
        created_for_b[0].deployment_id,
        DeploymentId::new("deployment-b"),
    );

    assert_eq!(
        created_for_b[1].deployment_id,
        DeploymentId::new("deployment-b"),
    );

    assert_eq!(state.instance_count(), 4);
}

#[test]
fn reconciling_missing_deployment_is_rejected() {
    let mut state = ControlState::new();

    let error = state
        .reconcile_deployment(&DeploymentId::new("missing-deployment"))
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::DeploymentNotFound(DeploymentId::new("missing-deployment",),),
    );

    assert_eq!(state.instance_count(), 0);
}

#[test]
fn scale_down_cancels_excess_replicas_deterministically() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    state
        .scale_deployment(&DeploymentId::new("deployment-01"), 1)
        .unwrap();

    let changed = state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert_eq!(changed.len(), 1);

    assert_eq!(changed[0].id, InstanceId::new("deployment-01-replica-1",),);

    assert_eq!(changed[0].status, InstanceStatus::Cancelled,);

    assert_eq!(
        state
            .instance(&InstanceId::new("deployment-01-replica-2",),)
            .unwrap()
            .status,
        InstanceStatus::Pending,
    );

    let repeated = state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert!(repeated.is_empty());
}

#[test]
fn scale_down_prefers_pending_replica_over_assigned_replica() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    let replicas = state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    state.register_node(node("node-01")).unwrap();

    state.schedule_instance(&replicas[1].id).unwrap();

    state
        .scale_deployment(&DeploymentId::new("deployment-01"), 1)
        .unwrap();

    let changed = state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert_eq!(changed.len(), 1);

    assert_eq!(changed[0].id, replicas[0].id,);

    assert_eq!(changed[0].status, InstanceStatus::Cancelled,);

    assert_eq!(
        state.instance(&replicas[1].id).unwrap().status,
        InstanceStatus::Assigned,
    );
}

#[test]
fn scale_down_to_zero_releases_reserved_node_capacity() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    let replicas = state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert_eq!(replicas.len(), 2);

    assert!(
        replicas
            .iter()
            .all(|instance| { instance.status == InstanceStatus::Assigned })
    );

    let node_before = state.node(&NodeId::new("node-01")).unwrap();

    assert_eq!(
        node_before.allocated,
        ResourceRequest::new(1_000, 134_217_728,),
    );

    assert_eq!(node_before.allocated_instances, 2,);

    state
        .scale_deployment(&DeploymentId::new("deployment-01"), 0)
        .unwrap();

    let changed = state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert_eq!(changed.len(), 2);

    assert!(
        changed
            .iter()
            .all(|instance| { instance.status == InstanceStatus::Cancelled })
    );

    let node_after = state.node(&NodeId::new("node-01")).unwrap();

    assert_eq!(node_after.allocated, ResourceRequest::default(),);

    assert_eq!(node_after.allocated_instances, 0,);
}

#[test]
fn terminal_instance_transition_releases_reserved_capacity() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    state
        .create_instance(instance("instance-01", "deployment-01", "workload-01"))
        .unwrap();

    state
        .assign_instance(&InstanceId::new("instance-01"), &NodeId::new("node-01"))
        .unwrap();

    state
        .transition_instance(&InstanceId::new("instance-01"), InstanceStatus::Failed)
        .unwrap();

    let node = state.node(&NodeId::new("node-01")).unwrap();

    assert_eq!(node.allocated, ResourceRequest::default(),);

    assert_eq!(node.allocated_instances, 0,);
}

#[test]
fn reconciliation_automatically_schedules_created_replicas() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    let changed = state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert_eq!(changed.len(), 2);

    assert!(
        changed
            .iter()
            .all(|instance| { instance.status == InstanceStatus::Assigned })
    );

    assert!(
        changed
            .iter()
            .all(|instance| { instance.node_id == Some(NodeId::new("node-01")) })
    );

    let node = state.node(&NodeId::new("node-01")).unwrap();

    assert_eq!(node.allocated, ResourceRequest::new(1_000, 134_217_728,),);

    assert_eq!(node.allocated_instances, 2,);
}

#[test]
fn reconciliation_keeps_replicas_pending_without_capacity() {
    let mut state = ControlState::new();

    let mut unavailable = node("node-01");
    unavailable.status = NodeStatus::Draining;

    state.register_node(unavailable).unwrap();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    let changed = state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert_eq!(changed.len(), 2);

    assert!(
        changed
            .iter()
            .all(|instance| { instance.status == InstanceStatus::Pending })
    );

    assert!(changed.iter().all(|instance| instance.node_id.is_none()));
}

#[test]
fn later_reconciliation_retries_existing_pending_replicas() {
    let mut state = ControlState::new();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    let first = state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert_eq!(first.len(), 2);

    assert!(
        first
            .iter()
            .all(|instance| { instance.status == InstanceStatus::Pending })
    );

    state.register_node(node("node-01")).unwrap();

    let second = state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert_eq!(second.len(), 2);

    assert!(
        second
            .iter()
            .all(|instance| { instance.status == InstanceStatus::Assigned })
    );

    assert_eq!(state.instance_count(), 2,);
}

#[test]
fn reconciliation_can_partially_schedule_when_capacity_is_limited() {
    let mut state = ControlState::new();

    let mut limited = node("node-01");
    limited.capacity.max_instances = 1;

    state.register_node(limited).unwrap();

    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    let changed = state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert_eq!(changed.len(), 2);

    assert_eq!(changed[0].status, InstanceStatus::Assigned,);

    assert_eq!(changed[1].status, InstanceStatus::Pending,);

    assert_eq!(changed[0].node_id, Some(NodeId::new("node-01")),);

    assert_eq!(changed[1].node_id, None,);

    let node = state.node(&NodeId::new("node-01")).unwrap();

    assert_eq!(node.allocated_instances, 1,);
}

#[test]
fn stale_worker_is_marked_unreachable_at_timeout_boundary() {
    use vessel_core::WorkerRegistration;

    let mut state = ControlState::new();

    state.register_worker(
        WorkerRegistration::new(node("node-stale"), "http://node-stale:7001"),
        1_000,
    );

    state.register_worker(
        WorkerRegistration::new(node("node-fresh"), "http://node-fresh:7001"),
        1_001,
    );

    state.register_node(node("manual-node")).unwrap();

    let changed = state.detect_stale_workers(6_000, 5_000);

    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].id, NodeId::new("node-stale"));
    assert_eq!(changed[0].status, NodeStatus::Unreachable);

    assert_eq!(
        state.node(&NodeId::new("node-stale")).unwrap().status,
        NodeStatus::Unreachable,
    );

    assert_eq!(
        state.node(&NodeId::new("node-fresh")).unwrap().status,
        NodeStatus::Ready,
    );

    assert_eq!(
        state.node(&NodeId::new("manual-node")).unwrap().status,
        NodeStatus::Ready,
    );
}

#[test]
fn stale_workers_are_reported_in_node_id_order() {
    use vessel_core::WorkerRegistration;

    let mut state = ControlState::new();

    for id in ["node-c", "node-a", "node-b"] {
        state.register_worker(
            WorkerRegistration::new(node(id), format!("http://{id}:7001")),
            1_000,
        );
    }

    let changed = state.detect_stale_workers(6_000, 5_000);

    let ids = changed
        .iter()
        .map(|node| node.id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(ids, vec!["node-a", "node-b", "node-c"]);
}

#[test]
fn stale_worker_detection_is_idempotent() {
    use vessel_core::WorkerRegistration;

    let mut state = ControlState::new();

    state.register_worker(
        WorkerRegistration::new(node("node-01"), "http://node-01:7001"),
        1_000,
    );

    let first = state.detect_stale_workers(6_000, 5_000);
    let second = state.detect_stale_workers(7_000, 5_000);

    assert_eq!(first.len(), 1);
    assert!(second.is_empty());

    assert_eq!(
        state.node(&NodeId::new("node-01")).unwrap().status,
        NodeStatus::Unreachable,
    );
}

#[test]
fn heartbeat_restores_worker_after_failure_detection() {
    use vessel_core::{ResourceRequest, WorkerHeartbeat, WorkerRegistration};

    let mut state = ControlState::new();

    let registered = node("node-01");

    state.register_worker(
        WorkerRegistration::new(registered.clone(), "http://node-01:7001"),
        1_000,
    );

    let changed = state.detect_stale_workers(6_000, 5_000);

    assert_eq!(changed.len(), 1);

    let heartbeat = WorkerHeartbeat {
        node_id: NodeId::new("node-01"),
        status: NodeStatus::Ready,
        capacity: registered.capacity,
        allocated: ResourceRequest::default(),
        allocated_instances: 0,
    };

    let restored = state.record_heartbeat(heartbeat, 7_000).unwrap();

    assert_eq!(restored.status, NodeStatus::Ready);

    assert_eq!(
        state.node_last_seen_ms(&NodeId::new("node-01")),
        Some(7_000),
    );

    let changed = state.detect_stale_workers(11_999, 5_000);

    assert!(changed.is_empty());

    assert_eq!(
        state.node(&NodeId::new("node-01")).unwrap().status,
        NodeStatus::Ready,
    );
}

#[test]
fn active_instances_on_failed_node_are_marked_lost() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();
    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    for id in ["instance-b", "instance-a"] {
        state
            .create_instance(instance(id, "deployment-01", "workload-01"))
            .unwrap();

        state
            .assign_instance(&InstanceId::new(id), &NodeId::new("node-01"))
            .unwrap();
    }

    let lost = state
        .mark_instances_lost_on_node(&NodeId::new("node-01"))
        .unwrap();

    assert_eq!(lost.len(), 2);

    assert_eq!(lost[0].id, InstanceId::new("instance-a"));
    assert_eq!(lost[1].id, InstanceId::new("instance-b"));

    assert!(
        lost.iter()
            .all(|instance| instance.status == InstanceStatus::Lost)
    );

    assert_eq!(
        state
            .node(&NodeId::new("node-01"))
            .unwrap()
            .allocated_instances,
        0,
    );
}

#[test]
fn recovery_marks_starting_and_running_instances_lost() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();
    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    state
        .create_instance(instance(
            "instance-starting",
            "deployment-01",
            "workload-01",
        ))
        .unwrap();

    state
        .assign_instance(
            &InstanceId::new("instance-starting"),
            &NodeId::new("node-01"),
        )
        .unwrap();

    state
        .transition_instance(
            &InstanceId::new("instance-starting"),
            InstanceStatus::Starting,
        )
        .unwrap();

    state
        .create_instance(instance("instance-running", "deployment-01", "workload-01"))
        .unwrap();

    state
        .assign_instance(
            &InstanceId::new("instance-running"),
            &NodeId::new("node-01"),
        )
        .unwrap();

    state
        .transition_instance(
            &InstanceId::new("instance-running"),
            InstanceStatus::Starting,
        )
        .unwrap();

    state
        .transition_instance(
            &InstanceId::new("instance-running"),
            InstanceStatus::Running,
        )
        .unwrap();

    let lost = state
        .mark_instances_lost_on_node(&NodeId::new("node-01"))
        .unwrap();

    assert_eq!(lost.len(), 2);

    assert!(
        lost.iter()
            .all(|instance| instance.status == InstanceStatus::Lost)
    );

    assert_eq!(
        state
            .node(&NodeId::new("node-01"))
            .unwrap()
            .allocated_instances,
        0,
    );
}

#[test]
fn recovery_only_affects_instances_on_failed_node() {
    let mut state = ControlState::new();

    state.register_node(node("node-a")).unwrap();
    state.register_node(node("node-b")).unwrap();
    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    state
        .create_instance(instance("instance-a", "deployment-01", "workload-01"))
        .unwrap();

    state
        .assign_instance(&InstanceId::new("instance-a"), &NodeId::new("node-a"))
        .unwrap();

    state
        .create_instance(instance("instance-b", "deployment-01", "workload-01"))
        .unwrap();

    state
        .assign_instance(&InstanceId::new("instance-b"), &NodeId::new("node-b"))
        .unwrap();

    let lost = state
        .mark_instances_lost_on_node(&NodeId::new("node-a"))
        .unwrap();

    assert_eq!(lost.len(), 1);
    assert_eq!(lost[0].id, InstanceId::new("instance-a"));

    assert_eq!(
        state
            .instance(&InstanceId::new("instance-a"))
            .unwrap()
            .status,
        InstanceStatus::Lost,
    );

    assert_eq!(
        state
            .instance(&InstanceId::new("instance-b"))
            .unwrap()
            .status,
        InstanceStatus::Assigned,
    );

    assert_eq!(
        state
            .node(&NodeId::new("node-a"))
            .unwrap()
            .allocated_instances,
        0,
    );

    assert_eq!(
        state
            .node(&NodeId::new("node-b"))
            .unwrap()
            .allocated_instances,
        1,
    );
}

#[test]
fn lost_instance_marking_is_idempotent_and_rejects_unknown_node() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();
    state.register_workload(workload("workload-01")).unwrap();

    state
        .create_deployment(deployment("deployment-01", "workload-01"))
        .unwrap();

    state
        .create_instance(instance("instance-01", "deployment-01", "workload-01"))
        .unwrap();

    state
        .assign_instance(&InstanceId::new("instance-01"), &NodeId::new("node-01"))
        .unwrap();

    let first = state
        .mark_instances_lost_on_node(&NodeId::new("node-01"))
        .unwrap();

    let second = state
        .mark_instances_lost_on_node(&NodeId::new("node-01"))
        .unwrap();

    assert_eq!(first.len(), 1);
    assert!(second.is_empty());

    let error = state
        .mark_instances_lost_on_node(&NodeId::new("missing-node"))
        .unwrap_err();

    assert_eq!(
        error,
        ControlError::NodeNotFound(NodeId::new("missing-node")),
    );
}

#[test]
fn failed_worker_replica_is_replaced_on_healthy_node() {
    use vessel_core::WorkerRegistration;

    let mut state = ControlState::new();

    state.register_worker(
        WorkerRegistration::new(node("node-a"), "http://node-a:7001"),
        1_000,
    );

    state.register_worker(
        WorkerRegistration::new(node("node-b"), "http://node-b:7001"),
        5_000,
    );

    state.register_workload(workload("workload-01")).unwrap();

    let mut deployment = deployment("deployment-01", "workload-01");

    deployment.desired_replicas = 1;

    state.create_deployment(deployment).unwrap();

    let initial = state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert_eq!(initial.len(), 1);

    assert_eq!(initial[0].id, InstanceId::new("deployment-01-replica-1"),);

    assert_eq!(initial[0].status, InstanceStatus::Assigned);

    assert_eq!(initial[0].node_id, Some(NodeId::new("node-a")),);

    let unreachable = state.detect_stale_workers(6_000, 5_000);

    assert_eq!(unreachable.len(), 1);
    assert_eq!(unreachable[0].id, NodeId::new("node-a"));

    let lost = state
        .mark_instances_lost_on_node(&NodeId::new("node-a"))
        .unwrap();

    assert_eq!(lost.len(), 1);

    assert_eq!(lost[0].id, InstanceId::new("deployment-01-replica-1"),);

    assert_eq!(lost[0].status, InstanceStatus::Lost);

    let replacements = state
        .reconcile_deployment(&DeploymentId::new("deployment-01"))
        .unwrap();

    assert_eq!(replacements.len(), 1);

    assert_eq!(
        replacements[0].id,
        InstanceId::new("deployment-01-replica-2"),
    );

    assert_eq!(replacements[0].status, InstanceStatus::Assigned,);

    assert_eq!(replacements[0].node_id, Some(NodeId::new("node-b")),);

    assert_eq!(
        state
            .instance(&InstanceId::new("deployment-01-replica-1",))
            .unwrap()
            .status,
        InstanceStatus::Lost,
    );

    assert_eq!(
        state.node(&NodeId::new("node-a")).unwrap().status,
        NodeStatus::Unreachable,
    );

    assert_eq!(
        state
            .node(&NodeId::new("node-a"))
            .unwrap()
            .allocated_instances,
        0,
    );

    assert_eq!(
        state
            .node(&NodeId::new("node-b"))
            .unwrap()
            .allocated_instances,
        1,
    );
}

#[test]
fn rolling_reconciliation_replaces_one_old_replica_per_pass() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();

    state.register_workload(workload("workload-v1")).unwrap();
    state.register_workload(workload("workload-v2")).unwrap();

    state
        .create_deployment(deployment("deployment-rollout", "workload-v1"))
        .unwrap();

    let initial = state
        .reconcile_deployment(&DeploymentId::new("deployment-rollout"))
        .unwrap();

    assert_eq!(initial.len(), 2);
    assert!(
        initial
            .iter()
            .all(|instance| { instance.status == InstanceStatus::Assigned })
    );

    state
        .rollout_deployment(
            &DeploymentId::new("deployment-rollout"),
            &WorkloadId::new("workload-v2"),
        )
        .unwrap();

    let first = state
        .reconcile_deployment(&DeploymentId::new("deployment-rollout"))
        .unwrap();

    assert_eq!(first.len(), 2);

    let active_after_first = state
        .list_instances()
        .into_iter()
        .filter(|instance| {
            instance.deployment_id == DeploymentId::new("deployment-rollout")
                && !instance.status.is_terminal()
        })
        .collect::<Vec<_>>();

    assert_eq!(active_after_first.len(), 2);

    assert_eq!(
        active_after_first
            .iter()
            .filter(|instance| { instance.workload_id == WorkloadId::new("workload-v1") })
            .count(),
        1,
    );

    assert_eq!(
        active_after_first
            .iter()
            .filter(|instance| { instance.workload_id == WorkloadId::new("workload-v2") })
            .count(),
        1,
    );

    assert_eq!(
        state
            .deployment(&DeploymentId::new("deployment-rollout",),)
            .unwrap()
            .status,
        DeploymentStatus::Progressing,
    );

    let second = state
        .reconcile_deployment(&DeploymentId::new("deployment-rollout"))
        .unwrap();

    assert_eq!(second.len(), 2);

    let active_after_second = state
        .list_instances()
        .into_iter()
        .filter(|instance| {
            instance.deployment_id == DeploymentId::new("deployment-rollout")
                && !instance.status.is_terminal()
        })
        .collect::<Vec<_>>();

    assert_eq!(active_after_second.len(), 2);

    assert!(active_after_second.iter().all(|instance| {
        instance.workload_id == WorkloadId::new("workload-v2")
            && instance.status == InstanceStatus::Assigned
    }));

    let deployment = state
        .deployment(&DeploymentId::new("deployment-rollout"))
        .unwrap();

    assert_eq!(deployment.generation, 2);
    assert_eq!(deployment.status, DeploymentStatus::Healthy,);

    let third = state
        .reconcile_deployment(&DeploymentId::new("deployment-rollout"))
        .unwrap();

    assert!(third.is_empty());
}

#[test]
fn rolling_reconciliation_does_not_drain_old_replicas_while_target_is_pending() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();

    state.register_workload(workload("workload-v1")).unwrap();

    let mut target = workload("workload-v2");
    target.spec.resources = ResourceRequest::new(5_000, 67_108_864);

    state.register_workload(target).unwrap();

    state
        .create_deployment(deployment("deployment-stalled", "workload-v1"))
        .unwrap();

    state
        .reconcile_deployment(&DeploymentId::new("deployment-stalled"))
        .unwrap();

    state
        .rollout_deployment(
            &DeploymentId::new("deployment-stalled"),
            &WorkloadId::new("workload-v2"),
        )
        .unwrap();

    let first = state
        .reconcile_deployment(&DeploymentId::new("deployment-stalled"))
        .unwrap();

    assert_eq!(first.len(), 2);

    assert!(
        first
            .iter()
            .any(|instance| { instance.status == InstanceStatus::Cancelled })
    );

    assert!(first.iter().any(|instance| {
        instance.workload_id == WorkloadId::new("workload-v2")
            && instance.status == InstanceStatus::Pending
    }));

    let second = state
        .reconcile_deployment(&DeploymentId::new("deployment-stalled"))
        .unwrap();

    assert!(second.is_empty());

    let instances = state.list_instances();

    let active_old = instances
        .iter()
        .filter(|instance| {
            instance.deployment_id == DeploymentId::new("deployment-stalled")
                && instance.workload_id == WorkloadId::new("workload-v1")
                && !instance.status.is_terminal()
        })
        .count();

    let pending_target = instances
        .iter()
        .filter(|instance| {
            instance.deployment_id == DeploymentId::new("deployment-stalled")
                && instance.workload_id == WorkloadId::new("workload-v2")
                && instance.status == InstanceStatus::Pending
        })
        .count();

    let cancelled_old = instances
        .iter()
        .filter(|instance| {
            instance.deployment_id == DeploymentId::new("deployment-stalled")
                && instance.workload_id == WorkloadId::new("workload-v1")
                && instance.status == InstanceStatus::Cancelled
        })
        .count();

    assert_eq!(active_old, 1);
    assert_eq!(pending_target, 1);
    assert_eq!(cancelled_old, 1);

    assert_eq!(
        state
            .deployment(&DeploymentId::new("deployment-stalled",),)
            .unwrap()
            .status,
        DeploymentStatus::Progressing,
    );
}

#[test]
fn rolling_reconciliation_resumes_after_pending_target_becomes_schedulable() {
    let mut state = ControlState::new();

    state.register_node(node("node-small")).unwrap();

    state.register_workload(workload("workload-v1")).unwrap();

    let mut target = workload("workload-v2");
    target.spec.resources = ResourceRequest::new(5_000, 67_108_864);

    state.register_workload(target).unwrap();

    state
        .create_deployment(deployment("deployment-resume", "workload-v1"))
        .unwrap();

    state
        .reconcile_deployment(&DeploymentId::new("deployment-resume"))
        .unwrap();

    state
        .rollout_deployment(
            &DeploymentId::new("deployment-resume"),
            &WorkloadId::new("workload-v2"),
        )
        .unwrap();

    state
        .reconcile_deployment(&DeploymentId::new("deployment-resume"))
        .unwrap();

    let mut large = node("node-large");
    large.capacity = ResourceCapacity::new(12_000, 536_870_912, 4);

    state.register_node(large).unwrap();

    let retry = state
        .reconcile_deployment(&DeploymentId::new("deployment-resume"))
        .unwrap();

    assert_eq!(retry.len(), 1);
    assert_eq!(retry[0].workload_id, WorkloadId::new("workload-v2"),);
    assert_eq!(retry[0].status, InstanceStatus::Assigned,);

    let completion = state
        .reconcile_deployment(&DeploymentId::new("deployment-resume"))
        .unwrap();

    assert_eq!(completion.len(), 2);

    let active = state
        .list_instances()
        .into_iter()
        .filter(|instance| {
            instance.deployment_id == DeploymentId::new("deployment-resume")
                && !instance.status.is_terminal()
        })
        .collect::<Vec<_>>();

    assert_eq!(active.len(), 2);

    assert!(active.iter().all(|instance| {
        instance.workload_id == WorkloadId::new("workload-v2")
            && instance.status == InstanceStatus::Assigned
    }));

    assert_eq!(
        state
            .deployment(&DeploymentId::new("deployment-resume",),)
            .unwrap()
            .status,
        DeploymentStatus::Healthy,
    );
}

#[test]
fn scale_down_during_rollout_prefers_previous_revision() {
    let mut state = ControlState::new();

    state.register_node(node("node-01")).unwrap();

    state.register_workload(workload("workload-v1")).unwrap();
    state.register_workload(workload("workload-v2")).unwrap();

    state
        .create_deployment(deployment("deployment-scale-rollout", "workload-v1"))
        .unwrap();

    state
        .reconcile_deployment(&DeploymentId::new("deployment-scale-rollout"))
        .unwrap();

    state
        .rollout_deployment(
            &DeploymentId::new("deployment-scale-rollout"),
            &WorkloadId::new("workload-v2"),
        )
        .unwrap();

    state
        .reconcile_deployment(&DeploymentId::new("deployment-scale-rollout"))
        .unwrap();

    state
        .scale_deployment(&DeploymentId::new("deployment-scale-rollout"), 1)
        .unwrap();

    let changed = state
        .reconcile_deployment(&DeploymentId::new("deployment-scale-rollout"))
        .unwrap();

    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].workload_id, WorkloadId::new("workload-v1"),);
    assert_eq!(changed[0].status, InstanceStatus::Cancelled,);

    let active = state
        .list_instances()
        .into_iter()
        .filter(|instance| {
            instance.deployment_id == DeploymentId::new("deployment-scale-rollout")
                && !instance.status.is_terminal()
        })
        .collect::<Vec<_>>();

    assert_eq!(active.len(), 1);
    assert_eq!(active[0].workload_id, WorkloadId::new("workload-v2"),);

    let deployment = state
        .deployment(&DeploymentId::new("deployment-scale-rollout"))
        .unwrap();

    assert_eq!(deployment.desired_replicas, 1);
    assert_eq!(deployment.generation, 3);
    assert_eq!(deployment.status, DeploymentStatus::Healthy,);
}
