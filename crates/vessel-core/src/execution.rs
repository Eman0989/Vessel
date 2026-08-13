use serde::{Deserialize, Serialize};

use crate::{NodeId, ResourceRequest};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionRequest {
    pub module_bytes: Vec<u8>,
    pub export: String,
    pub lhs: i32,
    pub rhs: i32,
    pub resources: ResourceRequest,
}

impl ExecutionRequest {
    pub fn new(
        module_bytes: impl Into<Vec<u8>>,
        export: impl Into<String>,
        lhs: i32,
        rhs: i32,
    ) -> Self {
        Self {
            module_bytes: module_bytes.into(),
            export: export.into(),
            lhs,
            rhs,
            resources: ResourceRequest::default(),
        }
    }

    pub fn with_resources(mut self, resources: ResourceRequest) -> Self {
        self.resources = resources;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionResult {
    pub node_id: NodeId,
    pub value: i32,
}
