use std::{
    collections::BTreeMap,
    sync::{Mutex, MutexGuard},
};

use reqwest::Client;
use sha2::{Digest, Sha256};
use thiserror::Error;
use vessel_core::ArtifactRef;

#[derive(Debug, Error)]
pub enum ArtifactCacheError {
    #[error("artifact cache lock was poisoned")]
    CachePoisoned,

    #[error("artifact digest is invalid: {digest}")]
    InvalidDigest { digest: String },

    #[error("artifact registry request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("artifact digest mismatch: expected {expected}, received {actual}")]
    DigestMismatch { expected: String, actual: String },
}

#[derive(Debug)]
pub struct ArtifactCache {
    client: Client,
    registry_url: String,
    entries: Mutex<BTreeMap<String, Vec<u8>>>,
}

impl ArtifactCache {
    pub fn new(registry_url: impl Into<String>) -> Self {
        let registry_url = registry_url.into();

        Self {
            client: Client::new(),
            registry_url: registry_url.trim_end_matches('/').to_string(),
            entries: Mutex::new(BTreeMap::new()),
        }
    }

    fn lock_entries(
        &self,
    ) -> Result<MutexGuard<'_, BTreeMap<String, Vec<u8>>>, ArtifactCacheError> {
        self.entries
            .lock()
            .map_err(|_| ArtifactCacheError::CachePoisoned)
    }

    pub fn digest_for(bytes: &[u8]) -> String {
        let digest = Sha256::digest(bytes);

        format!("sha256:{digest:x}")
    }

    fn validate_digest(digest: &str) -> Result<(), ArtifactCacheError> {
        let Some(hex) = digest.strip_prefix("sha256:") else {
            return Err(ArtifactCacheError::InvalidDigest {
                digest: digest.to_string(),
            });
        };

        let canonical = hex.len() == 64
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase());

        if !canonical {
            return Err(ArtifactCacheError::InvalidDigest {
                digest: digest.to_string(),
            });
        }

        Ok(())
    }

    pub async fn fetch(&self, artifact: &ArtifactRef) -> Result<Vec<u8>, ArtifactCacheError> {
        Self::validate_digest(&artifact.digest)?;

        if let Some(bytes) = self.lock_entries()?.get(&artifact.digest).cloned() {
            return Ok(bytes);
        }

        let response = self
            .client
            .get(format!(
                "{}/v1/artifacts/{}",
                self.registry_url, artifact.digest,
            ))
            .send()
            .await?
            .error_for_status()?;

        let bytes = response.bytes().await?.to_vec();

        let actual = Self::digest_for(&bytes);

        if actual != artifact.digest {
            return Err(ArtifactCacheError::DigestMismatch {
                expected: artifact.digest.clone(),
                actual,
            });
        }

        let mut entries = self.lock_entries()?;

        let cached = entries
            .entry(artifact.digest.clone())
            .or_insert(bytes)
            .clone();

        Ok(cached)
    }

    pub fn contains(&self, artifact: &ArtifactRef) -> Result<bool, ArtifactCacheError> {
        Ok(self.lock_entries()?.contains_key(&artifact.digest))
    }

    pub fn len(&self) -> Result<usize, ArtifactCacheError> {
        Ok(self.lock_entries()?.len())
    }

    pub fn is_empty(&self) -> Result<bool, ArtifactCacheError> {
        Ok(self.lock_entries()?.is_empty())
    }
}
