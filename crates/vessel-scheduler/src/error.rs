use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SchedulerError {
    #[error("no eligible node can satisfy request: cpu={cpu_millis}m, memory={memory_bytes} bytes")]
    NoEligibleNodes { cpu_millis: u32, memory_bytes: u64 },
}
