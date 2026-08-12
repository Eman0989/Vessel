use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PolicyError {
    #[error("environment variable name cannot be empty")]
    EmptyEnvironmentName,

    #[error("environment variable name `{name}` cannot contain `=`")]
    InvalidEnvironmentName { name: String },

    #[error("host filesystem path cannot be empty")]
    EmptyHostPath,

    #[error("guest filesystem path cannot be empty")]
    EmptyGuestPath,

    #[error("guest filesystem path `{path}` cannot contain parent traversal")]
    ParentTraversal { path: String },

    #[error("guest filesystem path `{path}` is configured more than once")]
    DuplicateGuestPath { path: String },
}
