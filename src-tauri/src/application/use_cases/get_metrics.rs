use std::sync::Arc;

use crate::application::use_cases::errors::ApplicationError;
use crate::infrastructure::metrics::collector::MetricsSnapshot;
use crate::infrastructure::metrics::MetricsCollector;

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

        let snapshot = self.metrics_collector.collect_metrics().map_err(|e| {
            ApplicationError::MetricsError {
                reason: format!("Failed to collect metrics: {}", e),
            }
        })?;

        tracing::debug!(
            target = "sapo_printer::use_case::get_metrics",
            total_jobs = snapshot.job_metrics.total_jobs,
            collected_at = snapshot.collected_at,
            "GetMetricsUseCase: completed"
        );

        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::print_job::aggregate::PrintJob;
    use crate::domain::print_job::value_objects::JobId;
    use crate::infrastructure::database::migrations::run_migrations;
    use crate::infrastructure::queue::{QueueError, QueueManager};
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
