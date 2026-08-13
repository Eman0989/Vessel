use std::collections::BTreeMap;

use vessel_core::{
    Deployment, DeploymentId, Instance, InstanceId, InstanceStatus, Node, NodeId, NodeStatus,
    Workload, WorkloadId,
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
        if !self.nodes.contains_key(node_id) {
            return Err(ControlError::NodeNotFound(node_id.clone()));
        }

        let instance = self
            .instances
            .get_mut(instance_id)
            .ok_or_else(|| ControlError::InstanceNotFound(instance_id.clone()))?;

        instance.assign_to(node_id.clone())?;

        Ok(instance.clone())
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

        let instance = self
            .instances
            .get_mut(instance_id)
            .ok_or_else(|| ControlError::InstanceNotFound(instance_id.clone()))?;

        instance.transition_to(status)?;

        Ok(instance.clone())
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
