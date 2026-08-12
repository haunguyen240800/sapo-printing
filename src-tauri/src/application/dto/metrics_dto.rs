use serde::Serialize;

use crate::application::ports::metrics_port::MetricsSnapshot;

#[derive(Clone, Debug, Serialize)]
pub struct MetricsDto {
    pub total_jobs: u64,
    pub completed: u64,
    pub failed: u64,
    pub avg_print_time_secs: f64,
}

impl From<MetricsSnapshot> for MetricsDto {
    fn from(snapshot: MetricsSnapshot) -> Self {
        Self {
            total_jobs: snapshot.job_metrics.total_jobs,
            completed: snapshot.job_metrics.completed,
            failed: snapshot.job_metrics.failed,
            avg_print_time_secs: snapshot.performance_metrics.avg_print_time_secs,
        }
    }
}
