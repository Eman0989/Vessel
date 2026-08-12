use std::time::Duration;
use wasmtime::{StoreLimits, StoreLimitsBuilder};

pub(crate) const EPOCH_TICK_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeLimits {
    pub fuel: u64,
    pub memory_bytes: usize,
    pub max_instances: usize,
    pub max_memories: usize,
    pub max_tables: usize,
    pub max_table_elements: usize,
    pub timeout: Duration,
}

impl Default for RuntimeLimits {
    fn default() -> Self {
        Self {
            fuel: 10_000_000,
            memory_bytes: 64 * 1024 * 1024,
            max_instances: 16,
            max_memories: 16,
            max_tables: 16,
            max_table_elements: 100_000,
            timeout: Duration::from_secs(5),
        }
    }
}

pub(crate) struct StoreState {
    pub(crate) limits: StoreLimits,
}

impl StoreState {
    pub(crate) fn new(config: RuntimeLimits) -> Self {
        let limits = StoreLimitsBuilder::new()
            .memory_size(config.memory_bytes)
            .instances(config.max_instances)
            .memories(config.max_memories)
            .tables(config.max_tables)
            .table_elements(config.max_table_elements)
            .trap_on_grow_failure(true)
            .build();

        Self { limits }
    }
}
