use serde::{Deserialize, Serialize};
use tracing_subscriber::{
    EnvFilter,
    util::{SubscriberInitExt, TryInitError},
};
use vessel_core::{Deployment, DeploymentStatus, Instance, InstanceStatus, Node, NodeStatus};

pub const DEFAULT_LOG_FILTER: &str = "info";
pub const PROMETHEUS_CONTENT_TYPE: &str = "text/plain; version=0.0.4; charset=utf-8";

/// Install VESSEL's process-wide structured tracing subscriber.
///
/// `RUST_LOG` controls filtering when present. Invalid or missing filters
/// fall back to the stable `info` default.
pub fn init_tracing(service_name: &str) -> Result<(), TryInitError> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_LOG_FILTER));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .compact()
        .finish()
        .try_init()?;

    tracing::info!(service = service_name, "VESSEL tracing initialized");

    Ok(())
}

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
    pub fn encode_prometheus(&self) -> String {
        let mut output = String::new();

        push_gauge_family(
            &mut output,
            "vessel_cluster_node_count",
            "Number of VESSEL worker nodes by state.",
            &[
                ("all", self.nodes.total),
                ("joining", self.nodes.joining),
                ("ready", self.nodes.ready),
                ("draining", self.nodes.draining),
                ("unreachable", self.nodes.unreachable),
            ],
        );

        push_gauge_family(
            &mut output,
            "vessel_cluster_deployment_count",
            "Number of VESSEL deployments by state.",
            &[
                ("all", self.deployments.total),
                ("pending", self.deployments.pending),
                ("progressing", self.deployments.progressing),
                ("healthy", self.deployments.healthy),
                ("degraded", self.deployments.degraded),
                ("failed", self.deployments.failed),
                ("stopped", self.deployments.stopped),
            ],
        );

        push_gauge(
            &mut output,
            "vessel_cluster_deployment_desired_replicas",
            "Total desired replicas across VESSEL deployments.",
            self.deployments.desired_replicas,
        );

        push_gauge(
            &mut output,
            "vessel_cluster_deployment_autoscaling_count",
            "Number of VESSEL deployments with autoscaling enabled.",
            self.deployments.autoscaling_enabled,
        );

        push_gauge(
            &mut output,
            "vessel_cluster_deployment_canary_count",
            "Number of VESSEL deployments with an active canary.",
            self.deployments.canary_active,
        );

        push_gauge_family(
            &mut output,
            "vessel_cluster_instance_count",
            "Number of VESSEL workload instances by state.",
            &[
                ("all", self.instances.total),
                ("active", self.instances.active),
                ("pending", self.instances.pending),
                ("assigned", self.instances.assigned),
                ("starting", self.instances.starting),
                ("running", self.instances.running),
                ("stopping", self.instances.stopping),
                ("succeeded", self.instances.succeeded),
                ("failed", self.instances.failed),
                ("lost", self.instances.lost),
                ("cancelled", self.instances.cancelled),
            ],
        );

        push_gauge(
            &mut output,
            "vessel_cluster_instance_restart_count",
            "Aggregate restart count across VESSEL instances.",
            self.instances.restart_count,
        );

        push_gauge_family(
            &mut output,
            "vessel_cluster_cpu_millis",
            "VESSEL cluster CPU capacity in millicores.",
            &[
                ("capacity", self.resources.capacity_cpu_millis),
                ("allocated", self.resources.allocated_cpu_millis),
                ("available", self.resources.available_cpu_millis),
            ],
        );

        push_gauge_family(
            &mut output,
            "vessel_cluster_memory_bytes",
            "VESSEL cluster memory capacity in bytes.",
            &[
                ("capacity", self.resources.capacity_memory_bytes),
                ("allocated", self.resources.allocated_memory_bytes),
                ("available", self.resources.available_memory_bytes),
            ],
        );

        push_gauge_family(
            &mut output,
            "vessel_cluster_instance_slots",
            "VESSEL cluster instance scheduling slots.",
            &[
                ("capacity", self.resources.max_instances),
                ("allocated", self.resources.allocated_instances),
                ("available", self.resources.available_instances),
            ],
        );

        output
    }

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

fn push_gauge(output: &mut String, name: &str, help: &str, value: u64) {
    output.push_str("# HELP ");
    output.push_str(name);
    output.push(' ');
    output.push_str(help);
    output.push('\n');

    output.push_str("# TYPE ");
    output.push_str(name);
    output.push_str(" gauge\n");

    output.push_str(name);
    output.push(' ');
    output.push_str(&value.to_string());
    output.push('\n');
}

fn push_gauge_family(output: &mut String, name: &str, help: &str, values: &[(&str, u64)]) {
    output.push_str("# HELP ");
    output.push_str(name);
    output.push(' ');
    output.push_str(help);
    output.push('\n');

    output.push_str("# TYPE ");
    output.push_str(name);
    output.push_str(" gauge\n");

    for (state, value) in values {
        output.push_str(name);
        output.push_str("{state=\"");
        output.push_str(state);
        output.push_str("\"} ");
        output.push_str(&value.to_string());
        output.push('\n');
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
    fn default_log_filter_is_valid() {
        assert!(EnvFilter::try_new(DEFAULT_LOG_FILTER).is_ok());
    }

    #[test]
    fn prometheus_text_has_stable_metric_families() {
        let metrics = ClusterMetrics {
            nodes: NodeMetrics {
                total: 2,
                ready: 1,
                unreachable: 1,
                ..NodeMetrics::default()
            },
            deployments: DeploymentMetrics {
                total: 1,
                healthy: 1,
                desired_replicas: 3,
                autoscaling_enabled: 1,
                ..DeploymentMetrics::default()
            },
            instances: InstanceMetrics {
                total: 3,
                active: 2,
                running: 2,
                failed: 1,
                restart_count: 4,
                ..InstanceMetrics::default()
            },
            resources: ResourceMetrics {
                capacity_cpu_millis: 4_000,
                allocated_cpu_millis: 1_000,
                available_cpu_millis: 3_000,
                capacity_memory_bytes: 8_000,
                allocated_memory_bytes: 2_000,
                available_memory_bytes: 6_000,
                max_instances: 8,
                allocated_instances: 3,
                available_instances: 5,
            },
        };

        let text = metrics.encode_prometheus();

        assert!(text.contains("vessel_cluster_node_count{state=\"ready\"} 1\n"));
        assert!(text.contains("vessel_cluster_deployment_desired_replicas 3\n"));
        assert!(text.contains("vessel_cluster_instance_count{state=\"running\"} 2\n"));
        assert!(text.contains("vessel_cluster_cpu_millis{state=\"available\"} 3000\n"));
        assert!(text.contains("vessel_cluster_memory_bytes{state=\"capacity\"} 8000\n"));
        assert!(text.contains("vessel_cluster_instance_slots{state=\"allocated\"} 3\n"));

        assert!(text.ends_with('\n'));
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
