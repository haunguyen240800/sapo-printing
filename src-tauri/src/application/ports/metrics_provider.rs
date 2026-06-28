//! MetricsProvider port — abstracts operational metrics collection.
//!
//! The use case depends on this trait; the SQLite-backed implementation lives
//! in the infrastructure layer.

use crate::shared::errors::InfrastructureError;

#[derive(Debug, Clone)]
pub struct JobMetrics {
    pub total_jobs: u64,
    pub pending: u64,
    pub queued: u64,
    pub downloaded: u64,
    pub submitted: u64,
    pub printing: u64,
    pub completed: u64,
    pub failed: u64,
    pub cancelled: u64,
    pub success_rate: f64,
}

#[derive(Debug, Clone)]
pub struct QueueMetrics {
    pub current_depth: usize,
    pub avg_wait_time_secs: f64,
}

#[derive(Debug, Clone)]
pub struct PrinterJobStats {
    pub printer_name: String,
    pub total_jobs: u64,
    pub completed_jobs: u64,
    pub utilization_percent: f64,
}

#[derive(Debug, Clone)]
pub struct PrinterMetrics {
    pub printers: Vec<PrinterJobStats>,
}

#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub avg_job_duration_secs: f64,
    pub p50_job_duration_secs: f64,
    pub p95_job_duration_secs: f64,
    pub p99_job_duration_secs: f64,
    pub avg_download_time_secs: f64,
    pub avg_render_time_secs: f64,
    pub avg_print_time_secs: f64,
}

#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub collected_at: i64,
    pub job_metrics: JobMetrics,
    pub queue_metrics: QueueMetrics,
    pub printer_metrics: PrinterMetrics,
    pub performance_metrics: PerformanceMetrics,
}

pub trait MetricsProvider: Send + Sync {
    fn collect(&self) -> Result<MetricsSnapshot, InfrastructureError>;
}
