use serde::Serialize;

use crate::application::errors::Error;

#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub total_jobs: u64,
    pub completed: u64,
    pub failed: u64,
    pub last_print_time_secs: f64,
}

pub trait MetricsPort: Send + Sync {
    fn collect(&self) -> Result<MetricsSnapshot, Error>;
}
