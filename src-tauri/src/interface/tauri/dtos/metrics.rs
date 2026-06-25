use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct MetricsDto {
    pub collected_at: i64,
    pub job_metrics: JobMetricsDto,
    pub queue_metrics: QueueMetricsDto,
    pub printer_metrics: PrinterMetricsDto,
    pub performance_metrics: PerformanceMetricsDto,
}

#[derive(Clone, Debug, Serialize)]
pub struct JobMetricsDto {
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

#[derive(Clone, Debug, Serialize)]
pub struct QueueMetricsDto {
    pub current_depth: usize,
    pub avg_wait_time_secs: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PrinterMetricsDto {
    pub printers: Vec<PrinterUsageDto>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PrinterUsageDto {
    pub printer_name: String,
    pub total_jobs: u64,
    pub completed_jobs: u64,
    pub utilization_percent: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PerformanceMetricsDto {
    pub avg_job_duration_secs: f64,
    pub p50_job_duration_secs: f64,
    pub p95_job_duration_secs: f64,
    pub p99_job_duration_secs: f64,
    pub avg_download_time_secs: f64,
    pub avg_render_time_secs: f64,
    pub avg_print_time_secs: f64,
}
