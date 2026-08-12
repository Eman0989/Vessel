use crate::{DeploymentId, WorkloadId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentStatus {
    Pending,
    Progressing,
    Healthy,
    Degraded,
    Failed,
    Stopped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Deployment {
    pub id: DeploymentId,
    pub workload_id: WorkloadId,
    pub desired_replicas: u32,
    pub generation: u64,
    pub status: DeploymentStatus,
}

impl Deployment {
    pub fn scale_to(&mut self, replicas: u32) {
        if self.desired_replicas != replicas {
            self.desired_replicas = replicas;
            self.generation += 1;
            self.status = DeploymentStatus::Progressing;
        }
    }

    pub fn is_converged(&self, running_replicas: u32) -> bool {
        running_replicas == self.desired_replicas
    }
}
