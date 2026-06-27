use std::sync::Arc;

use crate::application::use_cases::errors::ApplicationError;
use crate::infrastructure::telemetry::metrics::collector::MetricsSnapshot;
use crate::infrastructure::telemetry::metrics::MetricsCollector;

pub struct GetMetricsUseCase {
    metrics_collector: Arc<MetricsCollector>,
}

impl GetMetricsUseCase {
    pub fn new(metrics_collector: Arc<MetricsCollector>) -> Self {
        Self { metrics_collector }
    }

    pub fn execute(&self) -> Result<MetricsSnapshot, ApplicationError> {
        tracing::info!(
            target = "sapo_printer::use_case::get_metrics",
            "GetMetricsUseCase: starting"
        );

        // TEMPORARY FIX: Return empty metrics to prevent database lock contention
        // TODO: Investigate why metrics collection blocks indefinitely
        tracing::warn!(
            target = "sapo_printer::use_case::get_metrics",
            "GetMetricsUseCase: RETURNING EMPTY METRICS (temporary fix for deadlock)"
        );

        let collected_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let snapshot = crate::infrastructure::telemetry::metrics::collector::MetricsSnapshot {
            collected_at,
            job_metrics: crate::infrastructure::telemetry::metrics::collector::JobMetrics {
                total_jobs: 0,
                pending: 0,
                queued: 0,
                downloaded: 0,
                submitted: 0,
                printing: 0,
                completed: 0,
                failed: 0,
                cancelled: 0,
                success_rate: 0.0,
            },
            queue_metrics: crate::infrastructure::telemetry::metrics::collector::QueueMetrics {
                current_depth: 0,
                avg_wait_time_secs: 0.0,
            },
            printer_metrics: crate::infrastructure::telemetry::metrics::collector::PrinterMetrics {
                printers: vec![],
            },
            performance_metrics: crate::infrastructure::telemetry::metrics::collector::PerformanceMetrics {
                avg_job_duration_secs: 0.0,
                p50_job_duration_secs: 0.0,
                p95_job_duration_secs: 0.0,
                p99_job_duration_secs: 0.0,
                avg_download_time_secs: 0.0,
                avg_render_time_secs: 0.0,
                avg_print_time_secs: 0.0,
            },
        };

        tracing::info!(
            target = "sapo_printer::use_case::get_metrics",
            "GetMetricsUseCase: completed (empty metrics)"
        );

        Ok(snapshot)

        /* ORIGINAL CODE - RE-ENABLE AFTER FIXING DEADLOCK
        let start = std::time::Instant::now();
        let snapshot = self.metrics_collector.collect_metrics().map_err(|e| {
            ApplicationError::MetricsError {
                reason: format!("Failed to collect metrics: {}", e),
            }
        })?;
        let duration = start.elapsed();

        tracing::info!(
            target = "sapo_printer::use_case::get_metrics",
            duration_ms = duration.as_millis(),
            total_jobs = snapshot.job_metrics.total_jobs,
            "GetMetricsUseCase: completed"
        );

        if duration.as_secs() > 5 {
            tracing::warn!(
                target = "sapo_printer::use_case::get_metrics",
                duration_secs = duration.as_secs(),
                "GetMetricsUseCase: SLOW execution (>5s)"
            );
        }

        Ok(snapshot)
        */
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::PrintJob;
    use crate::domain::models::JobId;
    use crate::infrastructure::persistence::sqlite::migrations::run_migrations;
    use crate::infrastructure::persistence::task_queue::{QueueError, QueueManager};
    use rusqlite::Connection;
    use std::sync::Mutex as StdMutex;

    struct MockQueueManager;

    impl QueueManager for MockQueueManager {
        fn push(&self, _job_id: &JobId) -> Result<(), QueueError> { Ok(()) }
        fn pop(&self) -> Result<Option<PrintJob>, QueueError> { Ok(None) }
        fn requeue(&self, _job_id: &JobId, _delay_secs: u64) -> Result<(), QueueError> { Ok(()) }
        fn queue_depth(&self) -> Result<usize, QueueError> { Ok(0) }
    }

    #[test]
    fn test_execute_returns_snapshot() {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();
        let conn = Arc::new(StdMutex::new(conn));
        let qm = Arc::new(MockQueueManager);
        let collector = Arc::new(MetricsCollector::new(conn, qm));

        let use_case = GetMetricsUseCase::new(collector);
        let snapshot = use_case.execute().unwrap();

        assert_eq!(snapshot.job_metrics.total_jobs, 0);
        assert!(snapshot.collected_at > 0);
    }
}
