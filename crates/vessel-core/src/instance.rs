use crate::{CoreError, DeploymentId, InstanceId, NodeId, ResourceRequest, WorkloadId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstanceStatus {
    Pending,
    Assigned,
    Starting,
    Running,
    Stopping,
    Succeeded,
    Failed,
    Lost,
    Cancelled,
}

impl InstanceStatus {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Lost | Self::Cancelled
        )
    }

    pub fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Pending, Self::Assigned)
                | (Self::Pending, Self::Failed)
                | (Self::Pending, Self::Cancelled)
                | (Self::Assigned, Self::Starting)
                | (Self::Assigned, Self::Failed)
                | (Self::Assigned, Self::Lost)
                | (Self::Assigned, Self::Cancelled)
                | (Self::Starting, Self::Running)
                | (Self::Starting, Self::Failed)
                | (Self::Starting, Self::Lost)
                | (Self::Starting, Self::Cancelled)
                | (Self::Running, Self::Stopping)
                | (Self::Running, Self::Succeeded)
                | (Self::Running, Self::Failed)
                | (Self::Running, Self::Lost)
                | (Self::Running, Self::Cancelled)
                | (Self::Stopping, Self::Succeeded)
                | (Self::Stopping, Self::Failed)
                | (Self::Stopping, Self::Lost)
                | (Self::Stopping, Self::Cancelled)
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Instance {
    pub id: InstanceId,
    pub deployment_id: DeploymentId,
    pub workload_id: WorkloadId,
    pub node_id: Option<NodeId>,
    pub status: InstanceStatus,
    pub resources: ResourceRequest,
    pub restart_count: u32,
}

impl Instance {
    pub fn transition_to(&mut self, next: InstanceStatus) -> Result<(), CoreError> {
        if !self.status.can_transition_to(next) {
            return Err(CoreError::InvalidInstanceTransition {
                from: self.status,
                to: next,
            });
        }

        self.status = next;
        Ok(())
    }

    pub fn assign_to(&mut self, node_id: NodeId) -> Result<(), CoreError> {
        self.transition_to(InstanceStatus::Assigned)?;
        self.node_id = Some(node_id);
        Ok(())
    }
}
