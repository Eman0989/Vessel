use crate::{InstanceStatus, NodeId};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CoreError {
    #[error("node {node_id} is not schedulable")]
    NodeNotSchedulable { node_id: NodeId },

    #[error("node {node_id} does not have enough available capacity")]
    InsufficientCapacity { node_id: NodeId },

    #[error("invalid instance state transition from {from:?} to {to:?}")]
    InvalidInstanceTransition {
        from: InstanceStatus,
        to: InstanceStatus,
    },
}
