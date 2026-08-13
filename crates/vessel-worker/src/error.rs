use thiserror::Error;
use vessel_core::CoreError;
use vessel_runtime::RuntimeError;

use crate::ArtifactCacheError;

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("worker state lock was poisoned")]
    StatePoisoned,

    #[error(transparent)]
    Core(#[from] CoreError),

    #[error(transparent)]
    Runtime(#[from] RuntimeError),

    #[error(transparent)]
    ArtifactCache(#[from] ArtifactCacheError),
}
