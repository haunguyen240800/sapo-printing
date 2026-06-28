use crate::application::use_cases::GetMetricsUseCase;
use crate::interface::tauri::dtos::metrics::{
    JobMetricsDto, MetricsDto, PerformanceMetricsDto, PrinterMetricsDto, PrinterUsageDto,
    QueueMetricsDto,
};
use crate::AppContextState;

pub fn execute_get_metrics(ctx: &AppContextState) -> Result<MetricsDto, String> {
    let use_case = GetMetricsUseCase::new(ctx.metrics_provider.clone());
    let snapshot = use_case.execute().map_err(|e| format!("{}", e))?;

    Ok(MetricsDto {
        collected_at: snapshot.collected_at,
        job_metrics: JobMetricsDto {
            total_jobs: snapshot.job_metrics.total_jobs,
            pending: snapshot.job_metrics.pending,
            queued: snapshot.job_metrics.queued,
            downloaded: snapshot.job_metrics.downloaded,
            submitted: snapshot.job_metrics.submitted,
            printing: snapshot.job_metrics.printing,
            completed: snapshot.job_metrics.completed,
            failed: snapshot.job_metrics.failed,
            cancelled: snapshot.job_metrics.cancelled,
            success_rate: snapshot.job_metrics.success_rate,
        },
        queue_metrics: QueueMetricsDto {
            current_depth: snapshot.queue_metrics.current_depth,
            avg_wait_time_secs: snapshot.queue_metrics.avg_wait_time_secs,
        },
        printer_metrics: PrinterMetricsDto {
            printers: snapshot
                .printer_metrics
                .printers
                .iter()
                .map(|p| PrinterUsageDto {
                    printer_name: p.printer_name.clone(),
                    total_jobs: p.total_jobs,
                    completed_jobs: p.completed_jobs,
                    utilization_percent: p.utilization_percent,
                })
                .collect(),
        },
        performance_metrics: PerformanceMetricsDto {
            avg_job_duration_secs: snapshot.performance_metrics.avg_job_duration_secs,
            p50_job_duration_secs: snapshot.performance_metrics.p50_job_duration_secs,
            p95_job_duration_secs: snapshot.performance_metrics.p95_job_duration_secs,
            p99_job_duration_secs: snapshot.performance_metrics.p99_job_duration_secs,
            avg_download_time_secs: snapshot.performance_metrics.avg_download_time_secs,
            avg_render_time_secs: snapshot.performance_metrics.avg_render_time_secs,
            avg_print_time_secs: snapshot.performance_metrics.avg_print_time_secs,
        },
    })
}
