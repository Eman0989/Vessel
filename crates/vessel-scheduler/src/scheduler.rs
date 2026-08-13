use std::cmp::Ordering;

use vessel_core::{Node, ResourceCapacity, ResourceRequest};

use crate::{SchedulerError, SchedulingDecision, SchedulingScore};

#[derive(Debug, Default)]
pub struct Scheduler;

impl Scheduler {
    pub fn new() -> Self {
        Self
    }

    pub fn select_node(
        &self,
        nodes: &[Node],
        request: &ResourceRequest,
    ) -> Result<SchedulingDecision, SchedulerError> {
        nodes
            .iter()
            .filter(|node| node.can_schedule(request))
            .map(|node| candidate(node, request))
            .max_by(compare_preference)
            .ok_or(SchedulerError::NoEligibleNodes {
                cpu_millis: request.cpu_millis,
                memory_bytes: request.memory_bytes,
            })
    }

    pub fn rank_nodes(&self, nodes: &[Node], request: &ResourceRequest) -> Vec<SchedulingDecision> {
        let mut candidates = nodes
            .iter()
            .filter(|node| node.can_schedule(request))
            .map(|node| candidate(node, request))
            .collect::<Vec<_>>();

        candidates.sort_by(|left, right| compare_preference(right, left));

        candidates
    }
}

fn candidate(node: &Node, request: &ResourceRequest) -> SchedulingDecision {
    let available = node.available_capacity();

    let projected_capacity = ResourceCapacity::new(
        available.cpu_millis.saturating_sub(request.cpu_millis),
        available.memory_bytes.saturating_sub(request.memory_bytes),
        available.max_instances.saturating_sub(1),
    );

    SchedulingDecision {
        node_id: node.id.clone(),
        score: SchedulingScore {
            projected_capacity,
            allocated_instances: node.allocated_instances,
        },
    }
}

fn compare_preference(left: &SchedulingDecision, right: &SchedulingDecision) -> Ordering {
    left.score
        .projected_capacity
        .cpu_millis
        .cmp(&right.score.projected_capacity.cpu_millis)
        .then_with(|| {
            left.score
                .projected_capacity
                .memory_bytes
                .cmp(&right.score.projected_capacity.memory_bytes)
        })
        .then_with(|| {
            left.score
                .projected_capacity
                .max_instances
                .cmp(&right.score.projected_capacity.max_instances)
        })
        .then_with(|| {
            right
                .score
                .allocated_instances
                .cmp(&left.score.allocated_instances)
        })
        .then_with(|| right.node_id.cmp(&left.node_id))
}
