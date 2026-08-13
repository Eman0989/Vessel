use thiserror::Error;
use vessel_runtime::RuntimeError;

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
}
