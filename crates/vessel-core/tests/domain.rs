use std::collections::BTreeMap;

use vessel_core::{
    CoreError, Deployment, DeploymentId, DeploymentStatus, Instance, InstanceId, InstanceStatus,
    Node, NodeId, NodeStatus, ResourceCapacity, ResourceRequest, WorkloadId,
};

fn ready_node() -> Node {
    Node {
        id: NodeId::new("node-warsaw-01"),
        name: "warsaw-01".to_string(),
        region: "eu-central".to_string(),
        status: NodeStatus::Ready,
        capacity: ResourceCapacity::new(4_000, 8 * 1024 * 1024 * 1024, 16),
        allocated: ResourceRequest::default(),
        allocated_instances: 0,
        labels: BTreeMap::new(),
    }
}

#[test]
fn typed_ids_serialize_as_strings() {
    let id = NodeId::new("node-01");

    let json = serde_json::to_string(&id).unwrap();

    assert_eq!(json, "\"node-01\"");
}

#[test]
fn node_allocates_resources() {
    let mut node = ready_node();

    let request = ResourceRequest::new(500, 512 * 1024 * 1024);

    node.try_allocate(&request).unwrap();

    assert_eq!(node.allocated.cpu_millis, 500);
    assert_eq!(node.allocated.memory_bytes, 512 * 1024 * 1024);
    assert_eq!(node.allocated_instances, 1);
}

#[test]
fn node_rejects_resource_overcommit() {
    let mut node = ready_node();

    let request = ResourceRequest::new(8_000, 512 * 1024 * 1024);

    let result = node.try_allocate(&request);

    assert_eq!(
        result,
        Err(CoreError::InsufficientCapacity {
            node_id: NodeId::new("node-warsaw-01"),
        })
    );
}

#[test]
fn draining_node_cannot_accept_work() {
    let mut node = ready_node();
    node.status = NodeStatus::Draining;

    let request = ResourceRequest::new(100, 64 * 1024 * 1024);

    let result = node.try_allocate(&request);

    assert_eq!(
        result,
        Err(CoreError::NodeNotSchedulable {
            node_id: NodeId::new("node-warsaw-01"),
        })
    );
}

#[test]
fn deployment_scaling_increments_generation() {
    let mut deployment = Deployment {
        id: DeploymentId::new("deployment-01"),
        workload_id: WorkloadId::new("workload-01"),
        desired_replicas: 3,
        generation: 1,
        status: DeploymentStatus::Healthy,
        previous_workload_id: None,
        canary: None,
        autoscaling: None,
    };

    deployment.scale_to(8);

    assert_eq!(deployment.desired_replicas, 8);
    assert_eq!(deployment.generation, 2);
    assert_eq!(deployment.status, DeploymentStatus::Progressing);
}

#[test]
fn instance_follows_valid_state_transitions() {
    let mut instance = Instance {
        id: InstanceId::new("instance-01"),
        deployment_id: DeploymentId::new("deployment-01"),
        workload_id: WorkloadId::new("workload-01"),
        node_id: None,
        status: InstanceStatus::Pending,
        resources: ResourceRequest::new(250, 128 * 1024 * 1024),
        restart_count: 0,
    };

    instance.assign_to(NodeId::new("node-warsaw-01")).unwrap();

    instance.transition_to(InstanceStatus::Starting).unwrap();

    instance.transition_to(InstanceStatus::Running).unwrap();

    assert_eq!(instance.status, InstanceStatus::Running);

    assert_eq!(instance.node_id, Some(NodeId::new("node-warsaw-01")));
}

#[test]
fn invalid_instance_transition_is_rejected() {
    let mut instance = Instance {
        id: InstanceId::new("instance-01"),
        deployment_id: DeploymentId::new("deployment-01"),
        workload_id: WorkloadId::new("workload-01"),
        node_id: None,
        status: InstanceStatus::Pending,
        resources: ResourceRequest::new(250, 128 * 1024 * 1024),
        restart_count: 0,
    };

    let result = instance.transition_to(InstanceStatus::Running);

    assert_eq!(
        result,
        Err(CoreError::InvalidInstanceTransition {
            from: InstanceStatus::Pending,
            to: InstanceStatus::Running,
        })
    );
}
