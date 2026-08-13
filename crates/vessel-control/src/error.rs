use thiserror::Error;
use vessel_core::{DeploymentId, InstanceId, NodeId, WorkloadId};

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

    #[error("workload {0} was not found")]
    WorkloadNotFound(WorkloadId),

    #[error("deployment {0} was not found")]
    DeploymentNotFound(DeploymentId),

    #[error(
        "instance {instance_id} workload {instance_workload_id} does not match deployment workload {deployment_workload_id}"
    )]
    InstanceWorkloadMismatch {
        instance_id: InstanceId,
        instance_workload_id: WorkloadId,
        deployment_workload_id: WorkloadId,
    },
}
