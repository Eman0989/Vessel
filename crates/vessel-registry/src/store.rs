use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use vessel_core::ArtifactRef;

use crate::RegistryError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredArtifact {
    pub artifact: ArtifactRef,
    pub size_bytes: usize,
}

#[derive(Debug, Default)]
pub struct ArtifactStore {
    artifacts: BTreeMap<String, Vec<u8>>,
}

impl ArtifactStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn artifact_ref(bytes: &[u8]) -> ArtifactRef {
        let digest = Sha256::digest(bytes);

        ArtifactRef {
            digest: format!("sha256:{digest:x}"),
        }
    }

    pub fn put(&mut self, bytes: &[u8]) -> StoredArtifact {
        let artifact = Self::artifact_ref(bytes);

        self.artifacts
            .entry(artifact.digest.clone())
            .or_insert_with(|| bytes.to_vec());

        StoredArtifact {
            artifact,
            size_bytes: bytes.len(),
        }
    }

    pub fn get(&self, artifact: &ArtifactRef) -> Result<&[u8], RegistryError> {
        self.artifacts
            .get(&artifact.digest)
            .map(Vec::as_slice)
            .ok_or_else(|| RegistryError::ArtifactNotFound {
                digest: artifact.digest.clone(),
            })
    }

    pub fn contains(&self, artifact: &ArtifactRef) -> bool {
        self.artifacts.contains_key(&artifact.digest)
    }

    pub fn len(&self) -> usize {
        self.artifacts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.artifacts.is_empty()
    }
}
