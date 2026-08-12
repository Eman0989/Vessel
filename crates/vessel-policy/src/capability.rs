use crate::PolicyError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DirectoryAccess {
    ReadOnly,
    ReadWrite,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum NetworkAccess {
    #[default]
    DenyAll,
    AllowAll,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DirectoryCapability {
    pub host_path: PathBuf,
    pub guest_path: String,
    pub access: DirectoryAccess,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CapabilityPolicy {
    pub environment: BTreeMap<String, String>,
    pub directories: Vec<DirectoryCapability>,
    pub network: NetworkAccess,
}

impl CapabilityPolicy {
    pub fn deny_all() -> Self {
        Self::default()
    }

    pub fn validate(&self) -> Result<(), PolicyError> {
        self.validate_environment()?;
        self.validate_directories()?;

        Ok(())
    }

    fn validate_environment(&self) -> Result<(), PolicyError> {
        for name in self.environment.keys() {
            if name.is_empty() {
                return Err(PolicyError::EmptyEnvironmentName);
            }

            if name.contains('=') {
                return Err(PolicyError::InvalidEnvironmentName { name: name.clone() });
            }
        }

        Ok(())
    }

    fn validate_directories(&self) -> Result<(), PolicyError> {
        let mut guest_paths = BTreeSet::new();

        for directory in &self.directories {
            if directory.host_path.as_os_str().is_empty() {
                return Err(PolicyError::EmptyHostPath);
            }

            if directory.guest_path.is_empty() {
                return Err(PolicyError::EmptyGuestPath);
            }

            if directory
                .guest_path
                .split('/')
                .any(|segment| segment == "..")
            {
                return Err(PolicyError::ParentTraversal {
                    path: directory.guest_path.clone(),
                });
            }

            if !guest_paths.insert(directory.guest_path.clone()) {
                return Err(PolicyError::DuplicateGuestPath {
                    path: directory.guest_path.clone(),
                });
            }
        }

        Ok(())
    }
}
