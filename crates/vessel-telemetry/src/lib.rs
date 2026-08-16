use serde::{Deserialize, Serialize};
use vessel_core::{Deployment, DeploymentStatus, Instance, InstanceStatus, Node, NodeStatus};

/// Point-in-time aggregate telemetry derived from authoritative cluster state.
///
/// This type intentionally contains no wall-clock timestamp. Collection is
/// deterministic for a given state snapshot, while callers that expose or
/// persist metrics may attach observation time at the system boundary.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClusterMetrics {
    pub nodes: NodeMetrics,
    pub deployments: DeploymentMetrics,
    pub instances: InstanceMetrics,
    pub resources: ResourceMetrics,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeMetrics {
    pub total: u64,
    pub joining: u64,
    pub ready: u64,
    pub draining: u64,
    pub unreachable: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeploymentMetrics {
    pub total: u64,
    pub pending: u64,
    pub progressing: u64,
    pub healthy: u64,
    pub degraded: u64,
    pub failed: u64,
    pub stopped: u64,
    pub desired_replicas: u64,
    pub autoscaling_enabled: u64,
    pub canary_active: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstanceMetrics {
    pub total: u64,
    pub active: u64,
    pub pending: u64,
    pub assigned: u64,
    pub starting: u64,
    pub running: u64,
    pub stopping: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub lost: u64,
    pub cancelled: u64,
    pub restart_count: u64,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceMetrics {
    pub capacity_cpu_millis: u64,
    pub allocated_cpu_millis: u64,
    pub available_cpu_millis: u64,

    pub capacity_memory_bytes: u64,
    pub allocated_memory_bytes: u64,
    pub available_memory_bytes: u64,

    pub max_instances: u64,
    pub allocated_instances: u64,
    pub available_instances: u64,
}

impl ClusterMetrics {
    pub fn collect<'a>(
        nodes: impl IntoIterator<Item = &'a Node>,
        deployments: impl IntoIterator<Item = &'a Deployment>,
        instances: impl IntoIterator<Item = &'a Instance>,
    ) -> Self {
        let mut metrics = Self::default();

        for node in nodes {
            metrics.nodes.record(node);
            metrics.resources.record(node);
        }

        for deployment in deployments {
            metrics.deployments.record(deployment);
        }

        for instance in instances {
            metrics.instances.record(instance);
        }

        metrics
    }
}

impl NodeMetrics {
    fn record(&mut self, node: &Node) {
        self.total = self.total.saturating_add(1);

        match node.status {
            NodeStatus::Joining => {
                self.joining = self.joining.saturating_add(1);
            }
            NodeStatus::Ready => {
                self.ready = self.ready.saturating_add(1);
            }
            NodeStatus::Draining => {
                self.draining = self.draining.saturating_add(1);
            }
            NodeStatus::Unreachable => {
                self.unreachable = self.unreachable.saturating_add(1);
            }
        }
    }
}

impl DeploymentMetrics {
    fn record(&mut self, deployment: &Deployment) {
        self.total = self.total.saturating_add(1);

        match deployment.status {
            DeploymentStatus::Pending => {
                self.pending = self.pending.saturating_add(1);
            }
            DeploymentStatus::Progressing => {
                self.progressing = self.progressing.saturating_add(1);
            }
            DeploymentStatus::Healthy => {
                self.healthy = self.healthy.saturating_add(1);
            }
            DeploymentStatus::Degraded => {
                self.degraded = self.degraded.saturating_add(1);
            }
            DeploymentStatus::Failed => {
                self.failed = self.failed.saturating_add(1);
            }
            DeploymentStatus::Stopped => {
                self.stopped = self.stopped.saturating_add(1);
            }
        }

        self.desired_replicas = self
            .desired_replicas
            .saturating_add(u64::from(deployment.desired_replicas));

        if deployment.autoscaling.is_some() {
            self.autoscaling_enabled = self.autoscaling_enabled.saturating_add(1);
        }

        if deployment.canary.is_some() {
            self.canary_active = self.canary_active.saturating_add(1);
        }
    }
}

impl InstanceMetrics {
    fn record(&mut self, instance: &Instance) {
        self.total = self.total.saturating_add(1);

        if !instance.status.is_terminal() {
            self.active = self.active.saturating_add(1);
        }

        match instance.status {
            InstanceStatus::Pending => {
                self.pending = self.pending.saturating_add(1);
            }
            InstanceStatus::Assigned => {
                self.assigned = self.assigned.saturating_add(1);
            }
            InstanceStatus::Starting => {
                self.starting = self.starting.saturating_add(1);
            }
            InstanceStatus::Running => {
                self.running = self.running.saturating_add(1);
            }
            InstanceStatus::Stopping => {
                self.stopping = self.stopping.saturating_add(1);
            }
            InstanceStatus::Succeeded => {
                self.succeeded = self.succeeded.saturating_add(1);
            }
            InstanceStatus::Failed => {
                self.failed = self.failed.saturating_add(1);
            }
            InstanceStatus::Lost => {
                self.lost = self.lost.saturating_add(1);
            }
            InstanceStatus::Cancelled => {
                self.cancelled = self.cancelled.saturating_add(1);
            }
        }

        self.restart_count = self
            .restart_count
            .saturating_add(u64::from(instance.restart_count));
    }
}

impl ResourceMetrics {
    fn record(&mut self, node: &Node) {
        let available = node.available_capacity();

        self.capacity_cpu_millis = self
            .capacity_cpu_millis
            .saturating_add(u64::from(node.capacity.cpu_millis));

        self.allocated_cpu_millis = self
            .allocated_cpu_millis
            .saturating_add(u64::from(node.allocated.cpu_millis));

        self.available_cpu_millis = self
            .available_cpu_millis
            .saturating_add(u64::from(available.cpu_millis));

        self.capacity_memory_bytes = self
            .capacity_memory_bytes
            .saturating_add(node.capacity.memory_bytes);

        self.allocated_memory_bytes = self
            .allocated_memory_bytes
            .saturating_add(node.allocated.memory_bytes);

        self.available_memory_bytes = self
            .available_memory_bytes
            .saturating_add(available.memory_bytes);

        self.max_instances = self
            .max_instances
            .saturating_add(u64::from(node.capacity.max_instances));

        self.allocated_instances = self
            .allocated_instances
            .saturating_add(u64::from(node.allocated_instances));

        self.available_instances = self
            .available_instances
            .saturating_add(u64::from(available.max_instances));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use vessel_core::{
        AutoscalingPolicy, CanaryPlan, DeploymentId, InstanceId, NodeId, ResourceCapacity,
        ResourceRequest, WorkloadId,
    };

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

    fn deployment(id: &str, status: DeploymentStatus, desired_replicas: u32) -> Deployment {
        Deployment {
            id: DeploymentId::new(id),
            workload_id: WorkloadId::new("workload-v1"),
            desired_replicas,
            generation: 1,
            status,
            previous_workload_id: None,
            canary: None,
            autoscaling: None,
        }
    }

    fn instance(id: &str, status: InstanceStatus, restart_count: u32) -> Instance {
        Instance {
            id: InstanceId::new(id),
            deployment_id: DeploymentId::new("deployment-01"),
            workload_id: WorkloadId::new("workload-v1"),
            node_id: None,
            status,
            resources: ResourceRequest::new(100, 256),
            restart_count,
        }
    }

    #[test]
    fn empty_cluster_produces_zero_metrics() {
        let metrics = ClusterMetrics::collect(&[], &[], &[]);

        assert_eq!(metrics, ClusterMetrics::default());
    }

    #[test]
    fn node_and_resource_metrics_are_aggregated() {
        let nodes = vec![
            node(
                "node-ready",
                NodeStatus::Ready,
                ResourceCapacity::new(4_000, 8_000, 20),
                ResourceRequest::new(1_000, 2_000),
                4,
            ),
            node(
                "node-draining",
                NodeStatus::Draining,
                ResourceCapacity::new(2_000, 4_000, 10),
                ResourceRequest::new(500, 1_000),
                2,
            ),
            node(
                "node-joining",
                NodeStatus::Joining,
                ResourceCapacity::new(1_000, 2_000, 5),
                ResourceRequest::default(),
                0,
            ),
            node(
                "node-unreachable",
                NodeStatus::Unreachable,
                ResourceCapacity::new(1_000, 2_000, 5),
                ResourceRequest::default(),
                0,
            ),
        ];

        let metrics = ClusterMetrics::collect(&nodes, &[], &[]);

        assert_eq!(metrics.nodes.total, 4);
        assert_eq!(metrics.nodes.ready, 1);
        assert_eq!(metrics.nodes.draining, 1);
        assert_eq!(metrics.nodes.joining, 1);
        assert_eq!(metrics.nodes.unreachable, 1);

        assert_eq!(metrics.resources.capacity_cpu_millis, 8_000);
        assert_eq!(metrics.resources.allocated_cpu_millis, 1_500);
        assert_eq!(metrics.resources.available_cpu_millis, 6_500);

        assert_eq!(metrics.resources.capacity_memory_bytes, 16_000);
        assert_eq!(metrics.resources.allocated_memory_bytes, 3_000);
        assert_eq!(metrics.resources.available_memory_bytes, 13_000);

        assert_eq!(metrics.resources.max_instances, 40);
        assert_eq!(metrics.resources.allocated_instances, 6);
        assert_eq!(metrics.resources.available_instances, 34);
    }

    #[test]
    fn deployment_metrics_include_release_and_autoscaling_state() {
        let mut healthy = deployment("deployment-healthy", DeploymentStatus::Healthy, 3);

        healthy.autoscaling = Some(AutoscalingPolicy::new(1, 8, 70).unwrap());

        let mut progressing =
            deployment("deployment-progressing", DeploymentStatus::Progressing, 2);

        progressing.canary = Some(
            CanaryPlan::new(
                WorkloadId::new("workload-v1"),
                WorkloadId::new("workload-v2"),
                2,
                1,
            )
            .unwrap(),
        );

        let metrics = ClusterMetrics::collect(&[], &[healthy, progressing], &[]);

        assert_eq!(metrics.deployments.total, 2);
        assert_eq!(metrics.deployments.healthy, 1);
        assert_eq!(metrics.deployments.progressing, 1);
        assert_eq!(metrics.deployments.desired_replicas, 5);
        assert_eq!(metrics.deployments.autoscaling_enabled, 1);
        assert_eq!(metrics.deployments.canary_active, 1);
    }

    #[test]
    fn instance_metrics_track_lifecycle_and_restarts() {
        let instances = vec![
            instance("instance-running", InstanceStatus::Running, 2),
            instance("instance-pending", InstanceStatus::Pending, 0),
            instance("instance-lost", InstanceStatus::Lost, 1),
            instance("instance-cancelled", InstanceStatus::Cancelled, 0),
        ];

        let metrics = ClusterMetrics::collect(&[], &[], &instances);

        assert_eq!(metrics.instances.total, 4);
        assert_eq!(metrics.instances.active, 2);
        assert_eq!(metrics.instances.running, 1);
        assert_eq!(metrics.instances.pending, 1);
        assert_eq!(metrics.instances.lost, 1);
        assert_eq!(metrics.instances.cancelled, 1);
        assert_eq!(metrics.instances.restart_count, 3);
    }

    #[test]
    fn metrics_have_stable_json_shape() {
        let metrics = ClusterMetrics::default();

        let value = serde_json::to_value(metrics).unwrap();

        assert!(value["nodes"]["total"].is_number());
        assert!(value["deployments"]["desired_replicas"].is_number());
        assert!(value["instances"]["restart_count"].is_number());
        assert!(value["resources"]["capacity_cpu_millis"].is_number());
        assert!(value["resources"]["available_memory_bytes"].is_number());
        assert!(value["resources"]["available_instances"].is_number());
    }
}
