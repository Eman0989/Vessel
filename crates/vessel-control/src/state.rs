use std::collections::BTreeMap;

use vessel_core::{
    Deployment, DeploymentId, Instance, InstanceId, Node, NodeId, Workload, WorkloadId,
};

use crate::ControlError;

#[derive(Debug, Default)]
pub struct ControlState {
    nodes: BTreeMap<NodeId, Node>,
    workloads: BTreeMap<WorkloadId, Workload>,
    deployments: BTreeMap<DeploymentId, Deployment>,
    instances: BTreeMap<InstanceId, Instance>,
}

impl ControlState {
    pub fn new() -> Self {
        Self::default()
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
