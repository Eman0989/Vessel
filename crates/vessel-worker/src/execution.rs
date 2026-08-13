use vessel_core::NodeId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionRequest {
    pub module_bytes: Vec<u8>,
    pub export: String,
    pub lhs: i32,
    pub rhs: i32,
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionResult {
    pub node_id: NodeId,
    pub value: i32,
}
