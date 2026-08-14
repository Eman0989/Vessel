use std::collections::BTreeMap;

use vessel_core::{
    Deployment, DeploymentId, DeploymentStatus, Instance, InstanceId, InstanceStatus, Node, NodeId,
    NodeStatus, WorkerHeartbeat, WorkerRegistration, Workload, WorkloadId,
};

use vessel_scheduler::{Scheduler, SchedulerError};

use crate::ControlError;

#[derive(Debug, Default, Clone)]
pub struct ControlState {
    nodes: BTreeMap<NodeId, Node>,
    workloads: BTreeMap<WorkloadId, Workload>,
    deployments: BTreeMap<DeploymentId, Deployment>,
    instances: BTreeMap<InstanceId, Instance>,
    node_last_seen_ms: BTreeMap<NodeId, u64>,
    worker_endpoints: BTreeMap<NodeId, String>,
}

impl ControlState {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn restore_node_snapshot(
        &mut self,
        node: Node,
        endpoint: Option<String>,
        last_seen_ms: Option<u64>,
    ) -> Result<(), ControlError> {
        if self.nodes.contains_key(&node.id) {
            return Err(ControlError::NodeAlreadyExists(node.id.clone()));
        }

        let node_id = node.id.clone();

        self.nodes.insert(node_id.clone(), node);

        if let Some(endpoint) = endpoint {
            self.worker_endpoints.insert(node_id.clone(), endpoint);
        }

        if let Some(last_seen_ms) = last_seen_ms {
            self.node_last_seen_ms.insert(node_id, last_seen_ms);
        }

        Ok(())
    }

    pub fn register_worker(
        &mut self,
        registration: WorkerRegistration,
        observed_at_ms: u64,
    ) -> Node {
        let WorkerRegistration { node, endpoint } = registration;
        let node_id = node.id.clone();

        self.nodes.insert(node_id.clone(), node.clone());
        self.worker_endpoints.insert(node_id.clone(), endpoint);
        self.node_last_seen_ms.insert(node_id, observed_at_ms);

        node
    }

    pub fn record_heartbeat(
        &mut self,
        heartbeat: WorkerHeartbeat,
        observed_at_ms: u64,
    ) -> Result<Node, ControlError> {
        let node_id = heartbeat.node_id.clone();

        let updated = {
            let node = self
                .nodes
                .get_mut(&node_id)
                .ok_or_else(|| ControlError::NodeNotFound(node_id.clone()))?;

            node.status = heartbeat.status;
            node.capacity = heartbeat.capacity;
            node.allocated = heartbeat.allocated;
            node.allocated_instances = heartbeat.allocated_instances;

            node.clone()
        };

        self.node_last_seen_ms.insert(node_id, observed_at_ms);

        Ok(updated)
    }

    pub fn node_last_seen_ms(&self, id: &NodeId) -> Option<u64> {
        self.node_last_seen_ms.get(id).copied()
    }

    pub fn worker_endpoint(&self, id: &NodeId) -> Option<&str> {
        self.worker_endpoints.get(id).map(String::as_str)
    }

    pub fn detect_stale_workers(&mut self, observed_at_ms: u64, timeout_ms: u64) -> Vec<Node> {
        let stale_ids = self
            .node_last_seen_ms
            .iter()
            .filter(|(_, last_seen_ms)| observed_at_ms.saturating_sub(**last_seen_ms) >= timeout_ms)
            .map(|(node_id, _)| node_id.clone())
            .collect::<Vec<_>>();

        let mut changed = Vec::new();

        for node_id in stale_ids {
            if let Some(node) = self.nodes.get_mut(&node_id)
                && node.status != NodeStatus::Unreachable
            {
                node.status = NodeStatus::Unreachable;
                changed.push(node.clone());
            }
        }

        changed
    }

    pub fn register_node(&mut self, node: Node) -> Result<(), ControlError> {
        if self.nodes.contains_key(&node.id) {
            return Err(ControlError::NodeAlreadyExists(node.id.clone()));
        }

        self.nodes.insert(node.id.clone(), node);

        Ok(())
    }

    pub fn register_workload(&mut self, workload: Workload) -> Result<(), ControlError> {
        if self.workloads.contains_key(&workload.id) {
            return Err(ControlError::WorkloadAlreadyExists(workload.id.clone()));
        }

        self.workloads.insert(workload.id.clone(), workload);

        Ok(())
    }

    pub fn create_deployment(&mut self, deployment: Deployment) -> Result<(), ControlError> {
        if self.deployments.contains_key(&deployment.id) {
            return Err(ControlError::DeploymentAlreadyExists(deployment.id.clone()));
        }

        if !self.workloads.contains_key(&deployment.workload_id) {
            return Err(ControlError::WorkloadNotFound(
                deployment.workload_id.clone(),
            ));
        }

        self.deployments.insert(deployment.id.clone(), deployment);

        Ok(())
    }

    pub fn create_instance(&mut self, instance: Instance) -> Result<(), ControlError> {
        if self.instances.contains_key(&instance.id) {
            return Err(ControlError::InstanceAlreadyExists(instance.id.clone()));
        }

        let deployment = self
            .deployments
            .get(&instance.deployment_id)
            .ok_or_else(|| ControlError::DeploymentNotFound(instance.deployment_id.clone()))?;

        if deployment.workload_id != instance.workload_id {
            return Err(ControlError::InstanceWorkloadMismatch {
                instance_id: instance.id.clone(),
                instance_workload_id: instance.workload_id.clone(),
                deployment_workload_id: deployment.workload_id.clone(),
            });
        }

        self.instances.insert(instance.id.clone(), instance);

        Ok(())
    }

    pub fn node(&self, id: &NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn workload(&self, id: &WorkloadId) -> Option<&Workload> {
        self.workloads.get(id)
    }

    pub fn deployment(&self, id: &DeploymentId) -> Option<&Deployment> {
        self.deployments.get(id)
    }

    pub fn instance(&self, id: &InstanceId) -> Option<&Instance> {
        self.instances.get(id)
    }

    pub fn list_nodes(&self) -> Vec<Node> {
        self.nodes.values().cloned().collect()
    }

    pub fn list_workloads(&self) -> Vec<Workload> {
        self.workloads.values().cloned().collect()
    }

    pub fn list_deployments(&self) -> Vec<Deployment> {
        self.deployments.values().cloned().collect()
    }

    pub fn list_instances(&self) -> Vec<Instance> {
        self.instances.values().cloned().collect()
    }

    pub fn update_node_status(
        &mut self,
        id: &NodeId,
        status: NodeStatus,
    ) -> Result<Node, ControlError> {
        let node = self
            .nodes
            .get_mut(id)
            .ok_or_else(|| ControlError::NodeNotFound(id.clone()))?;

        node.status = status;

        Ok(node.clone())
    }

    fn next_replica_id(&self, deployment_id: &DeploymentId) -> InstanceId {
        let mut ordinal = 1_u64;

        loop {
            let candidate =
                InstanceId::new(format!("{}-replica-{ordinal}", deployment_id.as_str()));

            if !self.instances.contains_key(&candidate) {
                return candidate;
            }

            ordinal += 1;
        }
    }

    fn scale_down_priority(status: InstanceStatus) -> u8 {
        match status {
            InstanceStatus::Pending => 0,
            InstanceStatus::Assigned => 1,
            InstanceStatus::Starting => 2,
            InstanceStatus::Running => 3,
            InstanceStatus::Stopping => 4,
            InstanceStatus::Succeeded
            | InstanceStatus::Failed
            | InstanceStatus::Lost
            | InstanceStatus::Cancelled => 5,
        }
    }

    pub fn reconcile_deployment(
        &mut self,
        id: &DeploymentId,
    ) -> Result<Vec<Instance>, ControlError> {
        let deployment = self
            .deployments
            .get(id)
            .cloned()
            .ok_or_else(|| ControlError::DeploymentNotFound(id.clone()))?;

        let mut changed = BTreeMap::<InstanceId, Instance>::new();

        let mut active = self
            .instances
            .values()
            .filter(|instance| {
                instance.deployment_id == deployment.id && !instance.status.is_terminal()
            })
            .map(|instance| {
                (
                    Self::scale_down_priority(instance.status),
                    instance.id.clone(),
                )
            })
            .collect::<Vec<_>>();

        let active_replicas = active.len() as u32;

        if active_replicas < deployment.desired_replicas {
            let workload = self
                .workloads
                .get(&deployment.workload_id)
                .cloned()
                .ok_or_else(|| ControlError::WorkloadNotFound(deployment.workload_id.clone()))?;

            let missing_replicas = deployment.desired_replicas - active_replicas;

            for _ in 0..missing_replicas {
                let instance = Instance {
                    id: self.next_replica_id(&deployment.id),
                    deployment_id: deployment.id.clone(),
                    workload_id: deployment.workload_id.clone(),
                    node_id: None,
                    status: InstanceStatus::Pending,
                    resources: workload.spec.resources,
                    restart_count: 0,
                };

                self.instances.insert(instance.id.clone(), instance.clone());

                changed.insert(instance.id.clone(), instance);
            }
        } else if active_replicas > deployment.desired_replicas {
            let excess = active_replicas - deployment.desired_replicas;

            active.sort_by(|(left_priority, left_id), (right_priority, right_id)| {
                left_priority
                    .cmp(right_priority)
                    .then_with(|| left_id.cmp(right_id))
            });

            for (_, instance_id) in active.into_iter().take(excess as usize) {
                let instance = self.transition_instance(&instance_id, InstanceStatus::Cancelled)?;

                changed.insert(instance.id.clone(), instance);
            }
        }

        let pending_ids = self
            .instances
            .values()
            .filter(|instance| {
                instance.deployment_id == deployment.id
                    && instance.status == InstanceStatus::Pending
            })
            .map(|instance| instance.id.clone())
            .collect::<Vec<_>>();

        for instance_id in pending_ids {
            match self.schedule_instance(&instance_id) {
                Ok(instance) => {
                    changed.insert(instance.id.clone(), instance);
                }

                Err(ControlError::Scheduler(SchedulerError::NoEligibleNodes { .. })) => {
                    // Lack of cluster capacity is not a reconciliation
                    // failure. Keep this replica pending for a later pass.
                }

                Err(error) => return Err(error),
            }
        }

        if !changed.is_empty()
            && let Some(deployment) = self.deployments.get_mut(id)
        {
            deployment.status = DeploymentStatus::Progressing;
        }

        Ok(changed.into_values().collect())
    }

    pub fn scale_deployment(
        &mut self,
        id: &DeploymentId,
        replicas: u32,
    ) -> Result<Deployment, ControlError> {
        let deployment = self
            .deployments
            .get_mut(id)
            .ok_or_else(|| ControlError::DeploymentNotFound(id.clone()))?;

        deployment.scale_to(replicas);

        Ok(deployment.clone())
    }

    pub fn assign_instance(
        &mut self,
        instance_id: &InstanceId,
        node_id: &NodeId,
    ) -> Result<Instance, ControlError> {
        let resources = self
            .instances
            .get(instance_id)
            .ok_or_else(|| ControlError::InstanceNotFound(instance_id.clone()))?
            .resources;

        let node = self
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| ControlError::NodeNotFound(node_id.clone()))?;

        node.try_allocate(&resources)?;

        let assignment = {
            let instance = self
                .instances
                .get_mut(instance_id)
                .ok_or_else(|| ControlError::InstanceNotFound(instance_id.clone()))?;

            instance
                .assign_to(node_id.clone())
                .map(|()| instance.clone())
        };

        match assignment {
            Ok(instance) => Ok(instance),

            Err(error) => {
                node.release(&resources);
                Err(error.into())
            }
        }
    }

    pub fn mark_instances_lost_on_node(
        &mut self,
        node_id: &NodeId,
    ) -> Result<Vec<Instance>, ControlError> {
        if !self.nodes.contains_key(node_id) {
            return Err(ControlError::NodeNotFound(node_id.clone()));
        }

        let instance_ids = self
            .instances
            .values()
            .filter(|instance| {
                instance.node_id.as_ref() == Some(node_id)
                    && matches!(
                        instance.status,
                        InstanceStatus::Assigned
                            | InstanceStatus::Starting
                            | InstanceStatus::Running
                            | InstanceStatus::Stopping
                    )
            })
            .map(|instance| instance.id.clone())
            .collect::<Vec<_>>();

        let mut lost = Vec::with_capacity(instance_ids.len());

        for instance_id in instance_ids {
            lost.push(self.transition_instance(&instance_id, InstanceStatus::Lost)?);
        }

        Ok(lost)
    }

    pub fn schedule_instance(
        &mut self,
        instance_id: &InstanceId,
    ) -> Result<Instance, ControlError> {
        let resources = self
            .instances
            .get(instance_id)
            .ok_or_else(|| ControlError::InstanceNotFound(instance_id.clone()))?
            .resources;

        let nodes = self.list_nodes();

        let decision = Scheduler::new().select_node(&nodes, &resources)?;

        self.assign_instance(instance_id, &decision.node_id)
    }

    pub fn transition_instance(
        &mut self,
        instance_id: &InstanceId,
        status: InstanceStatus,
    ) -> Result<Instance, ControlError> {
        if status == InstanceStatus::Assigned {
            return Err(ControlError::InstanceAssignmentRequiresNode(
                instance_id.clone(),
            ));
        }

        let previous = self
            .instances
            .get(instance_id)
            .cloned()
            .ok_or_else(|| ControlError::InstanceNotFound(instance_id.clone()))?;

        let release_node_id = if !previous.status.is_terminal() && status.is_terminal() {
            previous.node_id.clone()
        } else {
            None
        };

        if let Some(node_id) = &release_node_id
            && !self.nodes.contains_key(node_id)
        {
            return Err(ControlError::NodeNotFound(node_id.clone()));
        }

        let updated = {
            let instance = self
                .instances
                .get_mut(instance_id)
                .ok_or_else(|| ControlError::InstanceNotFound(instance_id.clone()))?;

            instance.transition_to(status)?;
            instance.clone()
        };

        if let Some(node_id) = release_node_id {
            let node = self
                .nodes
                .get_mut(&node_id)
                .ok_or_else(|| ControlError::NodeNotFound(node_id.clone()))?;

            node.release(&previous.resources);
        }

        Ok(updated)
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn workload_count(&self) -> usize {
        self.workloads.len()
    }

    pub fn deployment_count(&self) -> usize {
        self.deployments.len()
    }

    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }
}
