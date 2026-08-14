use crate::{DeploymentId, WorkloadId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CanaryPlan {
    pub stable_workload_id: WorkloadId,
    pub candidate_workload_id: WorkloadId,
    pub candidate_replicas: u32,
}

#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum CanaryPlanError {
    #[error("canary candidate workload must differ from stable workload")]
    CandidateMatchesStable,

    #[error(
        "canary replica count must be between 1 and desired_replicas - 1: desired={desired_replicas}, candidate={candidate_replicas}"
    )]
    InvalidReplicaCount {
        desired_replicas: u32,
        candidate_replicas: u32,
    },
}

impl CanaryPlan {
    pub fn new(
        stable_workload_id: WorkloadId,
        candidate_workload_id: WorkloadId,
        desired_replicas: u32,
        candidate_replicas: u32,
    ) -> Result<Self, CanaryPlanError> {
        if stable_workload_id == candidate_workload_id {
            return Err(CanaryPlanError::CandidateMatchesStable);
        }

        Self::validate_replica_count(desired_replicas, candidate_replicas)?;

        Ok(Self {
            stable_workload_id,
            candidate_workload_id,
            candidate_replicas,
        })
    }

    pub fn stable_replicas(&self, desired_replicas: u32) -> Result<u32, CanaryPlanError> {
        Self::validate_replica_count(desired_replicas, self.candidate_replicas)?;

        Ok(desired_replicas - self.candidate_replicas)
    }

    fn validate_replica_count(
        desired_replicas: u32,
        candidate_replicas: u32,
    ) -> Result<(), CanaryPlanError> {
        if candidate_replicas == 0 || candidate_replicas >= desired_replicas {
            return Err(CanaryPlanError::InvalidReplicaCount {
                desired_replicas,
                candidate_replicas,
            });
        }

        Ok(())
    }
}

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

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_workload_id: Option<WorkloadId>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canary: Option<CanaryPlan>,
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
            let previous = std::mem::replace(&mut self.workload_id, workload_id);

            self.previous_workload_id = Some(previous);
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
            previous_workload_id: None,
            canary: None,
        }
    }

    #[test]
    fn canary_plan_preserves_stable_capacity() {
        let plan = CanaryPlan::new(
            WorkloadId::new("workload-v1"),
            WorkloadId::new("workload-v2"),
            4,
            1,
        )
        .unwrap();

        assert_eq!(plan.candidate_replicas, 1);
        assert_eq!(plan.stable_replicas(4).unwrap(), 3);
    }

    #[test]
    fn canary_plan_rejects_stable_revision_as_candidate() {
        let result = CanaryPlan::new(
            WorkloadId::new("workload-v1"),
            WorkloadId::new("workload-v1"),
            4,
            1,
        );

        assert_eq!(result, Err(CanaryPlanError::CandidateMatchesStable),);
    }

    #[test]
    fn canary_plan_rejects_zero_candidate_replicas() {
        let result = CanaryPlan::new(
            WorkloadId::new("workload-v1"),
            WorkloadId::new("workload-v2"),
            4,
            0,
        );

        assert_eq!(
            result,
            Err(CanaryPlanError::InvalidReplicaCount {
                desired_replicas: 4,
                candidate_replicas: 0,
            }),
        );
    }

    #[test]
    fn canary_plan_rejects_replacing_every_replica() {
        let result = CanaryPlan::new(
            WorkloadId::new("workload-v1"),
            WorkloadId::new("workload-v2"),
            4,
            4,
        );

        assert_eq!(
            result,
            Err(CanaryPlanError::InvalidReplicaCount {
                desired_replicas: 4,
                candidate_replicas: 4,
            }),
        );
    }

    #[test]
    fn canary_plan_detects_invalid_split_after_scale_down() {
        let plan = CanaryPlan::new(
            WorkloadId::new("workload-v1"),
            WorkloadId::new("workload-v2"),
            4,
            2,
        )
        .unwrap();

        assert_eq!(
            plan.stable_replicas(2),
            Err(CanaryPlanError::InvalidReplicaCount {
                desired_replicas: 2,
                candidate_replicas: 2,
            }),
        );
    }

    #[test]
    fn deployment_without_canary_field_deserializes_as_none() {
        let value = serde_json::json!({
            "id": "deployment-01",
            "workload_id": "workload-v1",
            "desired_replicas": 3,
            "generation": 1,
            "status": "healthy"
        });

        let deployment: Deployment = serde_json::from_value(value).unwrap();

        assert_eq!(deployment.previous_workload_id, None);
        assert_eq!(deployment.canary, None);
    }

    #[test]
    fn deployment_canary_state_round_trips_through_json() {
        let mut deployment = deployment();

        deployment.canary = Some(
            CanaryPlan::new(
                WorkloadId::new("workload-v1"),
                WorkloadId::new("workload-v2"),
                3,
                1,
            )
            .unwrap(),
        );

        let json = serde_json::to_string(&deployment).unwrap();

        let restored: Deployment = serde_json::from_str(&json).unwrap();

        assert_eq!(restored, deployment);
    }

    #[test]
    fn rollout_to_new_workload_advances_generation() {
        let mut deployment = deployment();

        deployment.rollout_to(WorkloadId::new("workload-v2"));

        assert_eq!(deployment.workload_id, WorkloadId::new("workload-v2"));

        assert_eq!(
            deployment.previous_workload_id,
            Some(WorkloadId::new("workload-v1")),
        );

        assert_eq!(deployment.generation, 2);
        assert_eq!(deployment.status, DeploymentStatus::Progressing);
    }

    #[test]
    fn rollout_to_current_workload_is_idempotent() {
        let mut deployment = deployment();

        deployment.rollout_to(WorkloadId::new("workload-v1"));

        assert_eq!(deployment.workload_id, WorkloadId::new("workload-v1"));
        assert_eq!(deployment.previous_workload_id, None);

        assert_eq!(deployment.generation, 1);
        assert_eq!(deployment.status, DeploymentStatus::Healthy);
    }

    #[test]
    fn successive_rollouts_retain_immediate_previous_revision() {
        let mut deployment = deployment();

        deployment.rollout_to(WorkloadId::new("workload-v2"));
        deployment.rollout_to(WorkloadId::new("workload-v3"));

        assert_eq!(deployment.workload_id, WorkloadId::new("workload-v3"),);

        assert_eq!(
            deployment.previous_workload_id,
            Some(WorkloadId::new("workload-v2")),
        );

        assert_eq!(deployment.generation, 3);
    }

    #[test]
    fn previous_workload_revision_round_trips_through_json() {
        let mut deployment = deployment();

        deployment.rollout_to(WorkloadId::new("workload-v2"));

        let json = serde_json::to_string(&deployment).unwrap();

        let restored: Deployment = serde_json::from_str(&json).unwrap();

        assert_eq!(
            restored.previous_workload_id,
            Some(WorkloadId::new("workload-v1")),
        );

        assert_eq!(restored, deployment);
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
