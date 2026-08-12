use crate::{ResourceRequest, WorkloadId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactRef {
    pub digest: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadStatus {
    Registered,
    Ready,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkloadSpec {
    pub name: String,
    pub artifact: ArtifactRef,
    pub resources: ResourceRequest,
    pub timeout_ms: u64,
    pub environment: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workload {
    pub id: WorkloadId,
    pub spec: WorkloadSpec,
    pub status: WorkloadStatus,
}
