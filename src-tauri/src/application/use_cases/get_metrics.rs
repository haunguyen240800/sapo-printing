use std::sync::Arc;

use crate::application::errors::ApplicationError;
use crate::application::ports::{MetricsProvider, MetricsSnapshot};

pub struct GetMetricsUseCase {
    metrics: Arc<dyn MetricsProvider>,
}

impl GetMetricsUseCase {
    pub fn new(metrics: Arc<dyn MetricsProvider>) -> Self {
        Self { metrics }
    }

    pub fn execute(&self) -> Result<MetricsSnapshot, ApplicationError> {
        tracing::info!(
            target = "sapo_printer::application::use_case::get_metrics",
            "GetMetricsUseCase: starting"
        );

        let start = std::time::Instant::now();
        let snapshot = self
            .metrics
            .collect()
            .map_err(|e| ApplicationError::MetricsError {
                reason: format!("Failed to collect metrics: {}", e),
            })?;
        let duration = start.elapsed();

        tracing::info!(
            target = "sapo_printer::application::use_case::get_metrics",
            duration_ms = duration.as_millis(),
            total_jobs = snapshot.job_metrics.total_jobs,
            "GetMetricsUseCase: completed"
        );

        if duration.as_secs() > 5 {
            tracing::warn!(
                target = "sapo_printer::application::use_case::get_metrics",
                duration_secs = duration.as_secs(),
                "GetMetricsUseCase: SLOW execution (>5s)"
            );
        }

        Ok(snapshot)
    }
}
