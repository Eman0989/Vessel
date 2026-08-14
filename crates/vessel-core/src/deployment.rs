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

    /// Point the deployment at a new immutable workload revision.
    ///
    /// Existing instances retain their original workload_id, allowing
    /// reconciliation to distinguish old replicas from target replicas
    /// during a rolling deployment.
    pub fn rollout_to(&mut self, workload_id: WorkloadId) {
        if self.workload_id != workload_id {
            self.workload_id = workload_id;
            self.generation += 1;
            self.status = DeploymentStatus::Progressing;
        }
    }

    pub fn is_converged(&self, running_replicas: u32) -> bool {
        running_replicas == self.desired_replicas
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deployment() -> Deployment {
        Deployment {
            id: DeploymentId::new("deployment-01"),
            workload_id: WorkloadId::new("workload-v1"),
            desired_replicas: 3,
            generation: 1,
            status: DeploymentStatus::Healthy,
        }
    }

    #[test]
    fn rollout_to_new_workload_advances_generation() {
        let mut deployment = deployment();

        deployment.rollout_to(WorkloadId::new("workload-v2"));

        assert_eq!(deployment.workload_id, WorkloadId::new("workload-v2"));

        assert_eq!(deployment.generation, 2);
        assert_eq!(deployment.status, DeploymentStatus::Progressing);
    }

    #[test]
    fn rollout_to_current_workload_is_idempotent() {
        let mut deployment = deployment();

        deployment.rollout_to(WorkloadId::new("workload-v1"));

        assert_eq!(deployment.workload_id, WorkloadId::new("workload-v1"));

        assert_eq!(deployment.generation, 1);
        assert_eq!(deployment.status, DeploymentStatus::Healthy);
    }

    #[test]
    fn scaling_and_rollout_advance_generation_independently() {
        let mut deployment = deployment();

        deployment.scale_to(5);
        deployment.rollout_to(WorkloadId::new("workload-v2"));

        assert_eq!(deployment.desired_replicas, 5);

        assert_eq!(deployment.workload_id, WorkloadId::new("workload-v2"));

        assert_eq!(deployment.generation, 3);
        assert_eq!(deployment.status, DeploymentStatus::Progressing);
    }
}
