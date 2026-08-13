use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RegistryError {
    #[error("artifact {digest} was not found")]
    ArtifactNotFound { digest: String },
}
