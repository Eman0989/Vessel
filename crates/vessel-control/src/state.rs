use std::collections::BTreeMap;

use vessel_core::{
    CanaryPlan, Deployment, DeploymentId, DeploymentStatus, Instance, InstanceId, InstanceStatus,
    Node, NodeId, NodeStatus, WorkerHeartbeat, WorkerRegistration, Workload, WorkloadId,
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

        if deployment.generation != 1
            || deployment.status != DeploymentStatus::Pending
            || deployment.previous_workload_id.is_some()
            || deployment.canary.is_some()
            || deployment.autoscaling.is_some()
        {
            return Err(ControlError::InvalidDeploymentInitialState(
                deployment.id.clone(),
            ));
        }

        self.deployments.insert(deployment.id.clone(), deployment);

        Ok(())
    }

    pub(crate) fn restore_deployment_snapshot(
        &mut self,
        deployment: Deployment,
    ) -> Result<(), ControlError> {
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

    pub(crate) fn restore_instance_snapshot(
        &mut self,
        instance: Instance,
    ) -> Result<(), ControlError> {
        if self.instances.contains_key(&instance.id) {
            return Err(ControlError::InstanceAlreadyExists(instance.id.clone()));
        }

        if !self.deployments.contains_key(&instance.deployment_id) {
            return Err(ControlError::DeploymentNotFound(
                instance.deployment_id.clone(),
            ));
        }

        if !self.workloads.contains_key(&instance.workload_id) {
            return Err(ControlError::WorkloadNotFound(instance.workload_id.clone()));
        }

        if let Some(node_id) = &instance.node_id
            && !self.nodes.contains_key(node_id)
        {
            return Err(ControlError::NodeNotFound(node_id.clone()));
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

    fn create_pending_replica_for_workload(
        &mut self,
        deployment: &Deployment,
        workload_id: &WorkloadId,
    ) -> Result<Instance, ControlError> {
        let workload = self
            .workloads
            .get(workload_id)
            .cloned()
            .ok_or_else(|| ControlError::WorkloadNotFound(workload_id.clone()))?;

        let instance = Instance {
            id: self.next_replica_id(&deployment.id),
            deployment_id: deployment.id.clone(),
            workload_id: workload_id.clone(),
            node_id: None,
            status: InstanceStatus::Pending,
            resources: workload.spec.resources,
            restart_count: 0,
        };

        self.instances.insert(instance.id.clone(), instance.clone());

        Ok(instance)
    }

    fn create_pending_replica(
        &mut self,
        deployment: &Deployment,
    ) -> Result<Instance, ControlError> {
        self.create_pending_replica_for_workload(deployment, &deployment.workload_id)
    }

    fn rollout_replica_available(status: InstanceStatus) -> bool {
        matches!(
            status,
            InstanceStatus::Assigned | InstanceStatus::Starting | InstanceStatus::Running
        )
    }

    fn reconcile_canary_deployment(
        &mut self,
        deployment: &Deployment,
        canary: &CanaryPlan,
    ) -> Result<Vec<Instance>, ControlError> {
        let stable_target = canary.stable_replicas(deployment.desired_replicas)?;
        let candidate_target = canary.candidate_replicas;

        let mut changed = BTreeMap::<InstanceId, Instance>::new();

        let snapshot = self
            .instances
            .values()
            .filter(|instance| {
                instance.deployment_id == deployment.id && !instance.status.is_terminal()
            })
            .map(|instance| {
                (
                    instance.id.clone(),
                    instance.workload_id.clone(),
                    instance.status,
                )
            })
            .collect::<Vec<_>>();

        let active_total = snapshot.len() as u32;

        let stable_count = snapshot
            .iter()
            .filter(|(_, workload_id, _)| workload_id == &canary.stable_workload_id)
            .count() as u32;

        let candidate_count = snapshot
            .iter()
            .filter(|(_, workload_id, _)| workload_id == &canary.candidate_workload_id)
            .count() as u32;

        let stable_deficit = stable_target.saturating_sub(stable_count);
        let candidate_deficit = candidate_target.saturating_sub(candidate_count);

        let stable_excess = stable_count.saturating_sub(stable_target);
        let candidate_excess = candidate_count.saturating_sub(candidate_target);

        let mut unexpected = snapshot
            .iter()
            .filter(|(_, workload_id, _)| {
                workload_id != &canary.stable_workload_id
                    && workload_id != &canary.candidate_workload_id
            })
            .map(|(id, _, status)| (Self::scale_down_priority(*status), id.clone()))
            .collect::<Vec<_>>();

        let replacement_workload_id = if stable_deficit > 0 {
            Some(canary.stable_workload_id.clone())
        } else if candidate_deficit > 0 {
            Some(canary.candidate_workload_id.clone())
        } else {
            None
        };

        let replacement_in_flight = replacement_workload_id.as_ref().is_some_and(|target| {
            snapshot.iter().any(|(_, workload_id, status)| {
                workload_id == target && !Self::rollout_replica_available(*status)
            })
        });

        if !unexpected.is_empty() {
            unexpected.sort_by(|(left_priority, left_id), (right_priority, right_id)| {
                left_priority
                    .cmp(right_priority)
                    .then_with(|| left_id.cmp(right_id))
            });

            let (_, victim_id) = unexpected
                .into_iter()
                .next()
                .expect("unexpected replica list is not empty");

            let removed = self.transition_instance(&victim_id, InstanceStatus::Cancelled)?;

            changed.insert(removed.id.clone(), removed);

            if active_total <= deployment.desired_replicas
                && !replacement_in_flight
                && let Some(workload_id) = replacement_workload_id.as_ref()
            {
                let replacement =
                    self.create_pending_replica_for_workload(deployment, workload_id)?;

                changed.insert(replacement.id.clone(), replacement);
            }
        } else {
            let mut excess_replicas = Vec::new();

            if stable_excess > 0 {
                excess_replicas.extend(
                    snapshot
                        .iter()
                        .filter(|(_, workload_id, _)| workload_id == &canary.stable_workload_id)
                        .map(|(id, _, status)| (Self::scale_down_priority(*status), id.clone())),
                );
            }

            if candidate_excess > 0 {
                excess_replicas.extend(
                    snapshot
                        .iter()
                        .filter(|(_, workload_id, _)| workload_id == &canary.candidate_workload_id)
                        .map(|(id, _, status)| (Self::scale_down_priority(*status), id.clone())),
                );
            }

            excess_replicas.sort_by(|(left_priority, left_id), (right_priority, right_id)| {
                left_priority
                    .cmp(right_priority)
                    .then_with(|| left_id.cmp(right_id))
            });

            if active_total > deployment.desired_replicas {
                if let Some((_, victim_id)) = excess_replicas.into_iter().next() {
                    let removed =
                        self.transition_instance(&victim_id, InstanceStatus::Cancelled)?;

                    changed.insert(removed.id.clone(), removed);
                }
            } else if !excess_replicas.is_empty() {
                if !replacement_in_flight {
                    let (_, victim_id) = excess_replicas
                        .into_iter()
                        .next()
                        .expect("excess replica list is not empty");

                    let removed =
                        self.transition_instance(&victim_id, InstanceStatus::Cancelled)?;

                    changed.insert(removed.id.clone(), removed);

                    if let Some(workload_id) = replacement_workload_id.as_ref() {
                        let replacement =
                            self.create_pending_replica_for_workload(deployment, workload_id)?;

                        changed.insert(replacement.id.clone(), replacement);
                    }
                }
            } else if active_total < deployment.desired_replicas {
                for _ in 0..stable_deficit {
                    let instance = self.create_pending_replica_for_workload(
                        deployment,
                        &canary.stable_workload_id,
                    )?;

                    changed.insert(instance.id.clone(), instance);
                }

                for _ in 0..candidate_deficit {
                    let instance = self.create_pending_replica_for_workload(
                        deployment,
                        &canary.candidate_workload_id,
                    )?;

                    changed.insert(instance.id.clone(), instance);
                }
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

                Err(ControlError::Scheduler(SchedulerError::NoEligibleNodes { .. })) => {}

                Err(error) => return Err(error),
            }
        }

        let active_instances = self
            .instances
            .values()
            .filter(|instance| {
                instance.deployment_id == deployment.id && !instance.status.is_terminal()
            })
            .collect::<Vec<_>>();

        let stable_after = active_instances
            .iter()
            .filter(|instance| instance.workload_id == canary.stable_workload_id)
            .count() as u32;

        let candidate_after = active_instances
            .iter()
            .filter(|instance| instance.workload_id == canary.candidate_workload_id)
            .count() as u32;

        let only_expected_revisions = active_instances.iter().all(|instance| {
            instance.workload_id == canary.stable_workload_id
                || instance.workload_id == canary.candidate_workload_id
        });

        let all_available = active_instances
            .iter()
            .all(|instance| Self::rollout_replica_available(instance.status));

        let converged = active_instances.len() as u32 == deployment.desired_replicas
            && stable_after == stable_target
            && candidate_after == candidate_target
            && only_expected_revisions
            && all_available;

        if let Some(stored) = self.deployments.get_mut(&deployment.id) {
            stored.status = if converged {
                DeploymentStatus::Healthy
            } else {
                DeploymentStatus::Progressing
            };
        }

        Ok(changed.into_values().collect())
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

        if let Some(canary) = &deployment.canary {
            return self.reconcile_canary_deployment(&deployment, canary);
        }

        let mut changed = BTreeMap::<InstanceId, Instance>::new();

        let mut active = self
            .instances
            .values()
            .filter(|instance| {
                instance.deployment_id == deployment.id && !instance.status.is_terminal()
            })
            .map(|instance| {
                let revision_priority = if instance.workload_id == deployment.workload_id {
                    1_u8
                } else {
                    0_u8
                };

                (
                    revision_priority,
                    Self::scale_down_priority(instance.status),
                    instance.id.clone(),
                )
            })
            .collect::<Vec<_>>();

        let active_replicas = active.len() as u32;

        if active_replicas < deployment.desired_replicas {
            let missing_replicas = deployment.desired_replicas - active_replicas;

            for _ in 0..missing_replicas {
                let instance = self.create_pending_replica(&deployment)?;

                changed.insert(instance.id.clone(), instance);
            }
        } else if active_replicas > deployment.desired_replicas {
            let excess = active_replicas - deployment.desired_replicas;

            // During a rollout, scale-down removes replicas
            // from previous workload revisions before removing
            // target-revision replicas.
            active.sort_by(
                |(left_revision, left_priority, left_id),
                 (right_revision, right_priority, right_id)| {
                    left_revision
                        .cmp(right_revision)
                        .then_with(|| left_priority.cmp(right_priority))
                        .then_with(|| left_id.cmp(right_id))
                },
            );

            for (_, _, instance_id) in active.into_iter().take(excess as usize) {
                let instance = self.transition_instance(&instance_id, InstanceStatus::Cancelled)?;

                changed.insert(instance.id.clone(), instance);
            }
        } else {
            let mut old_replicas = self
                .instances
                .values()
                .filter(|instance| {
                    instance.deployment_id == deployment.id
                        && !instance.status.is_terminal()
                        && instance.workload_id != deployment.workload_id
                })
                .map(|instance| {
                    (
                        Self::scale_down_priority(instance.status),
                        instance.id.clone(),
                    )
                })
                .collect::<Vec<_>>();

            // A target replica that has not yet become
            // available represents the single allowed
            // in-flight replacement. Do not cancel another
            // old replica until it becomes available.
            let replacement_in_flight = self.instances.values().any(|instance| {
                instance.deployment_id == deployment.id
                    && !instance.status.is_terminal()
                    && instance.workload_id == deployment.workload_id
                    && !Self::rollout_replica_available(instance.status)
            });

            if !old_replicas.is_empty() && !replacement_in_flight {
                old_replicas.sort_by(|(left_priority, left_id), (right_priority, right_id)| {
                    left_priority
                        .cmp(right_priority)
                        .then_with(|| left_id.cmp(right_id))
                });

                let (_, old_instance_id) = old_replicas
                    .into_iter()
                    .next()
                    .expect("old replica list is not empty");

                let old_instance =
                    self.transition_instance(&old_instance_id, InstanceStatus::Cancelled)?;

                changed.insert(old_instance.id.clone(), old_instance);

                let replacement = self.create_pending_replica(&deployment)?;

                changed.insert(replacement.id.clone(), replacement);
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
                    // Lack of cluster capacity is not a
                    // reconciliation failure. The pending
                    // target replica also blocks another
                    // rollout replacement on the next pass.
                }

                Err(error) => return Err(error),
            }
        }

        let active_instances = self
            .instances
            .values()
            .filter(|instance| {
                instance.deployment_id == deployment.id && !instance.status.is_terminal()
            })
            .collect::<Vec<_>>();

        let deployment_converged = active_instances.len() as u32 == deployment.desired_replicas
            && active_instances.iter().all(|instance| {
                instance.workload_id == deployment.workload_id
                    && Self::rollout_replica_available(instance.status)
            });

        if let Some(stored) = self.deployments.get_mut(id) {
            if deployment_converged {
                stored.status = DeploymentStatus::Healthy;
            } else if !changed.is_empty() || stored.status == DeploymentStatus::Progressing {
                stored.status = DeploymentStatus::Progressing;
            }
        }

        Ok(changed.into_values().collect())
    }

    pub fn begin_canary_deployment(
        &mut self,
        id: &DeploymentId,
        candidate_workload_id: &WorkloadId,
        candidate_replicas: u32,
    ) -> Result<Deployment, ControlError> {
        let current = self
            .deployments
            .get(id)
            .cloned()
            .ok_or_else(|| ControlError::DeploymentNotFound(id.clone()))?;

        if !self.workloads.contains_key(candidate_workload_id) {
            return Err(ControlError::WorkloadNotFound(
                candidate_workload_id.clone(),
            ));
        }

        let plan = CanaryPlan::new(
            current.workload_id.clone(),
            candidate_workload_id.clone(),
            current.desired_replicas,
            candidate_replicas,
        )?;

        if let Some(active) = &current.canary {
            if active == &plan {
                return Ok(current);
            }

            return Err(ControlError::CanaryAlreadyActive(id.clone()));
        }

        if current.status != DeploymentStatus::Healthy {
            return Err(ControlError::CanaryRequiresHealthyDeployment {
                deployment_id: id.clone(),
                status: current.status,
            });
        }

        let deployment = self
            .deployments
            .get_mut(id)
            .ok_or_else(|| ControlError::DeploymentNotFound(id.clone()))?;

        deployment.canary = Some(plan);
        deployment.generation += 1;
        deployment.status = DeploymentStatus::Progressing;

        Ok(deployment.clone())
    }

    pub fn promote_canary_deployment(
        &mut self,
        id: &DeploymentId,
    ) -> Result<Deployment, ControlError> {
        let current = self
            .deployments
            .get(id)
            .cloned()
            .ok_or_else(|| ControlError::DeploymentNotFound(id.clone()))?;

        let canary = current
            .canary
            .clone()
            .ok_or_else(|| ControlError::CanaryNotActive(id.clone()))?;

        if current.status != DeploymentStatus::Healthy {
            return Err(ControlError::CanaryNotReady {
                deployment_id: id.clone(),
                status: current.status,
            });
        }

        if !self.workloads.contains_key(&canary.candidate_workload_id) {
            return Err(ControlError::WorkloadNotFound(canary.candidate_workload_id));
        }

        let deployment = self
            .deployments
            .get_mut(id)
            .ok_or_else(|| ControlError::DeploymentNotFound(id.clone()))?;

        deployment.canary = None;
        deployment.rollout_to(canary.candidate_workload_id);

        Ok(deployment.clone())
    }

    pub fn rollback_deployment(&mut self, id: &DeploymentId) -> Result<Deployment, ControlError> {
        let current = self
            .deployments
            .get(id)
            .cloned()
            .ok_or_else(|| ControlError::DeploymentNotFound(id.clone()))?;

        if current.canary.is_some() {
            let deployment = self
                .deployments
                .get_mut(id)
                .ok_or_else(|| ControlError::DeploymentNotFound(id.clone()))?;

            deployment.canary = None;
            deployment.generation += 1;
            deployment.status = DeploymentStatus::Progressing;

            return Ok(deployment.clone());
        }

        let previous_workload_id = current
            .previous_workload_id
            .clone()
            .ok_or_else(|| ControlError::RollbackUnavailable(id.clone()))?;

        if !self.workloads.contains_key(&previous_workload_id) {
            return Err(ControlError::WorkloadNotFound(previous_workload_id));
        }

        let deployment = self
            .deployments
            .get_mut(id)
            .ok_or_else(|| ControlError::DeploymentNotFound(id.clone()))?;

        deployment.rollout_to(previous_workload_id);

        Ok(deployment.clone())
    }

    pub fn rollout_deployment(
        &mut self,
        id: &DeploymentId,
        workload_id: &WorkloadId,
    ) -> Result<Deployment, ControlError> {
        if !self.deployments.contains_key(id) {
            return Err(ControlError::DeploymentNotFound(id.clone()));
        }

        if !self.workloads.contains_key(workload_id) {
            return Err(ControlError::WorkloadNotFound(workload_id.clone()));
        }

        if self
            .deployments
            .get(id)
            .and_then(|deployment| deployment.canary.as_ref())
            .is_some()
        {
            return Err(ControlError::CanaryAlreadyActive(id.clone()));
        }

        let deployment = self
            .deployments
            .get_mut(id)
            .ok_or_else(|| ControlError::DeploymentNotFound(id.clone()))?;

        deployment.rollout_to(workload_id.clone());

        Ok(deployment.clone())
    }

    pub fn scale_deployment(
        &mut self,
        id: &DeploymentId,
        replicas: u32,
    ) -> Result<Deployment, ControlError> {
        let current = self
            .deployments
            .get(id)
            .cloned()
            .ok_or_else(|| ControlError::DeploymentNotFound(id.clone()))?;

        if let Some(canary) = &current.canary {
            canary.stable_replicas(replicas)?;
        }

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
