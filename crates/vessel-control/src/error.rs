use thiserror::Error;
use vessel_core::{CoreError, DeploymentId, InstanceId, NodeId, WorkloadId};
use vessel_scheduler::SchedulerError;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ControlError {
    #[error("node {0} already exists")]
    NodeAlreadyExists(NodeId),

    #[error("workload {0} already exists")]
    WorkloadAlreadyExists(WorkloadId),

    #[error("deployment {0} already exists")]
    DeploymentAlreadyExists(DeploymentId),

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
