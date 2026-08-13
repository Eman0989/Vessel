use std::collections::BTreeMap;

use vessel_core::{Node, NodeId, NodeStatus, ResourceCapacity, ResourceRequest};
use vessel_scheduler::{Scheduler, SchedulerError};

fn node(
    id: &str,
    status: NodeStatus,
    capacity: ResourceCapacity,
    allocated: ResourceRequest,
    allocated_instances: u32,
) -> Node {
    Node {
        id: NodeId::new(id),
        name: id.to_string(),
        region: "test".to_string(),
        status,
        capacity,
        allocated,
        allocated_instances,
        labels: BTreeMap::new(),
    }
}

#[test]
fn ready_node_with_capacity_is_selected() {
    let scheduler = Scheduler::new();

    let nodes = vec![node(
        "node-a",
        NodeStatus::Ready,
        ResourceCapacity::new(2_000, 2_000, 4),
        ResourceRequest::default(),
        0,
    )];

    let decision = scheduler
        .select_node(&nodes, &ResourceRequest::new(500, 500))
        .unwrap();

    assert_eq!(decision.node_id, NodeId::new("node-a"),);

    assert_eq!(
        decision.score.projected_capacity,
        ResourceCapacity::new(1_500, 1_500, 3,),
    );
}

#[test]
fn non_ready_nodes_are_filtered_out() {
    let scheduler = Scheduler::new();

    let capacity = ResourceCapacity::new(2_000, 2_000, 4);

    let nodes = vec![
        node(
            "joining",
            NodeStatus::Joining,
            capacity,
            ResourceRequest::default(),
            0,
        ),
        node(
            "draining",
            NodeStatus::Draining,
            capacity,
            ResourceRequest::default(),
            0,
        ),
        node(
            "unreachable",
            NodeStatus::Unreachable,
            capacity,
            ResourceRequest::default(),
            0,
        ),
        node(
            "ready",
            NodeStatus::Ready,
            capacity,
            ResourceRequest::default(),
            0,
        ),
    ];

    let decision = scheduler
        .select_node(&nodes, &ResourceRequest::new(100, 100))
        .unwrap();

    assert_eq!(decision.node_id, NodeId::new("ready"),);
}

#[test]
fn nodes_without_capacity_are_filtered_out() {
    let scheduler = Scheduler::new();

    let nodes = vec![node(
        "small-node",
        NodeStatus::Ready,
        ResourceCapacity::new(100, 100, 1),
        ResourceRequest::default(),
        0,
    )];

    let error = scheduler
        .select_node(&nodes, &ResourceRequest::new(200, 50))
        .unwrap_err();

    assert_eq!(
        error,
        SchedulerError::NoEligibleNodes {
            cpu_millis: 200,
            memory_bytes: 50,
        },
    );
}

#[test]
fn scheduler_prefers_more_remaining_cpu() {
    let scheduler = Scheduler::new();

    let nodes = vec![
        node(
            "node-a",
            NodeStatus::Ready,
            ResourceCapacity::new(2_000, 4_000, 4),
            ResourceRequest::new(1_000, 0),
            1,
        ),
        node(
            "node-b",
            NodeStatus::Ready,
            ResourceCapacity::new(2_000, 4_000, 4),
            ResourceRequest::new(200, 0),
            1,
        ),
    ];

    let decision = scheduler
        .select_node(&nodes, &ResourceRequest::new(500, 500))
        .unwrap();

    assert_eq!(decision.node_id, NodeId::new("node-b"),);
}

#[test]
fn scheduler_uses_memory_when_cpu_score_ties() {
    let scheduler = Scheduler::new();

    let nodes = vec![
        node(
            "node-a",
            NodeStatus::Ready,
            ResourceCapacity::new(2_000, 4_000, 4),
            ResourceRequest::new(500, 3_000),
            1,
        ),
        node(
            "node-b",
            NodeStatus::Ready,
            ResourceCapacity::new(2_000, 4_000, 4),
            ResourceRequest::new(500, 1_000),
            1,
        ),
    ];

    let decision = scheduler
        .select_node(&nodes, &ResourceRequest::new(500, 500))
        .unwrap();

    assert_eq!(decision.node_id, NodeId::new("node-b"),);
}

#[test]
fn scheduler_prefers_lower_instance_load_after_capacity_ties() {
    let scheduler = Scheduler::new();

    let nodes = vec![
        node(
            "node-a",
            NodeStatus::Ready,
            ResourceCapacity::new(2_000, 4_000, 4),
            ResourceRequest::new(500, 500),
            2,
        ),
        node(
            "node-b",
            NodeStatus::Ready,
            ResourceCapacity::new(2_000, 4_000, 3),
            ResourceRequest::new(500, 500),
            1,
        ),
    ];

    let decision = scheduler
        .select_node(&nodes, &ResourceRequest::new(500, 500))
        .unwrap();

    assert_eq!(decision.node_id, NodeId::new("node-b"),);
}

#[test]
fn node_id_breaks_complete_ties_deterministically() {
    let scheduler = Scheduler::new();

    let capacity = ResourceCapacity::new(2_000, 4_000, 4);

    let nodes = vec![
        node(
            "node-z",
            NodeStatus::Ready,
            capacity,
            ResourceRequest::default(),
            0,
        ),
        node(
            "node-a",
            NodeStatus::Ready,
            capacity,
            ResourceRequest::default(),
            0,
        ),
    ];

    let decision = scheduler
        .select_node(&nodes, &ResourceRequest::new(500, 500))
        .unwrap();

    assert_eq!(decision.node_id, NodeId::new("node-a"),);
}

#[test]
fn ranked_nodes_are_best_first_and_exclude_ineligible_nodes() {
    let scheduler = Scheduler::new();

    let nodes = vec![
        node(
            "node-low",
            NodeStatus::Ready,
            ResourceCapacity::new(1_000, 2_000, 2),
            ResourceRequest::default(),
            0,
        ),
        node(
            "node-high",
            NodeStatus::Ready,
            ResourceCapacity::new(3_000, 4_000, 4),
            ResourceRequest::default(),
            0,
        ),
        node(
            "node-draining",
            NodeStatus::Draining,
            ResourceCapacity::new(9_000, 9_000, 9),
            ResourceRequest::default(),
            0,
        ),
    ];

    let ranked = scheduler.rank_nodes(&nodes, &ResourceRequest::new(500, 500));

    assert_eq!(ranked.len(), 2);

    assert_eq!(ranked[0].node_id, NodeId::new("node-high"),);

    assert_eq!(ranked[1].node_id, NodeId::new("node-low"),);
}
