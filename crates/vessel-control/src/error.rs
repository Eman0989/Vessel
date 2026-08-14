use thiserror::Error;
use vessel_core::{
    CanaryPlanError, CoreError, DeploymentId, DeploymentStatus, InstanceId, NodeId, WorkloadId,
};
use vessel_scheduler::SchedulerError;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ControlError {
    #[error("node {0} already exists")]
    NodeAlreadyExists(NodeId),

    #[error("workload {0} already exists")]
    WorkloadAlreadyExists(WorkloadId),

    #[error("deployment {0} already exists")]
    DeploymentAlreadyExists(DeploymentId),

    #[error(
        "deployment {0} must start at generation 1 in pending state without rollback history, an active canary, or an autoscaling policy"
    )]
    InvalidDeploymentInitialState(DeploymentId),

    #[error("instance {0} already exists")]
    InstanceAlreadyExists(InstanceId),

    #[error("node {0} was not found")]
    NodeNotFound(NodeId),

    #[error("workload {0} was not found")]
    WorkloadNotFound(WorkloadId),

    #[error("deployment {0} was not found")]
    DeploymentNotFound(DeploymentId),

    #[error("instance {0} was not found")]
    InstanceNotFound(InstanceId),

    #[error("deployment {0} already has an active canary")]
    CanaryAlreadyActive(DeploymentId),

    #[error(
        "deployment {deployment_id} must be healthy before starting a canary; current status is {status:?}"
    )]
    CanaryRequiresHealthyDeployment {
        deployment_id: DeploymentId,
        status: DeploymentStatus,
    },

    #[error("deployment {0} does not have an active canary")]
    CanaryNotActive(DeploymentId),

    #[error(
        "deployment {deployment_id} canary is not ready for promotion; current status is {status:?}"
    )]
    CanaryNotReady {
        deployment_id: DeploymentId,
        status: DeploymentStatus,
    },

    #[error("deployment {0} has no rollback workload revision")]
    RollbackUnavailable(DeploymentId),

    #[error(transparent)]
    CanaryPlan(#[from] CanaryPlanError),

    #[error(
        "instance {instance_id} workload {instance_workload_id} does not match deployment workload {deployment_workload_id}"
    )]
    InstanceWorkloadMismatch {
        instance_id: InstanceId,
        instance_workload_id: WorkloadId,
        deployment_workload_id: WorkloadId,
    },

    #[error("instance {0} must be assigned through assign_instance so that a node is recorded")]
    InstanceAssignmentRequiresNode(InstanceId),

    #[error(transparent)]
    Core(#[from] CoreError),

    #[error(transparent)]
    Scheduler(#[from] SchedulerError),
}
