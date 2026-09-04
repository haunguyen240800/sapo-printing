use std::sync::Arc;

use crate::application::errors::Error;
use crate::application::ports::EventStore;
use crate::application::ports::event_bus::EventBus;
use crate::domain::print_job::PrintJobRepository;

pub struct CancelPrintJobUseCase {
    pub job_repo: Arc<dyn PrintJobRepository>,
    pub event_store: Arc<dyn EventStore>,
    pub event_bus: Arc<dyn EventBus>,
}

impl CancelPrintJobUseCase {
    pub fn new(
        job_repo: Arc<dyn PrintJobRepository>,
        event_store: Arc<dyn EventStore>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            job_repo,
            event_store,
            event_bus,
        }
    }

    /// Trả về số job đã bị hủy trong lần gọi này.
    pub fn execute(&self, slip_id: &str) -> Result<usize, Error> {
        if slip_id.trim().is_empty() {
            return Err(Error::ValidationError {
                reason: "slip_id must not be empty".to_string(),
            });
        }

        tracing::info!(
            target = "sapo_printer::application::use_case::cancel_print_job",
            slip_id = slip_id,
            "CancelPrintJobUseCase: starting"
        );

        let jobs = self
            .job_repo
            .find_by_slip_id(slip_id)
            .map_err(|e| Error::RepositoryError(e.to_string()))?;

        let mut cancelled = 0usize;
        for mut job in jobs {
            if job.status().is_terminal() {
                continue;
            }

            // cancel() chỉ từ chối các trạng thái terminal; đã lọc ở trên nên an toàn.
            if let Err(e) = job.cancel() {
                tracing::warn!(
                    target = "sapo_printer::application::use_case::cancel_print_job",
                    slip_id = slip_id,
                    job_id = %job.id(),
                    error = %e,
                    "Skip job that cannot be cancelled"
                );
                continue;
            }

            let events = job.drain_events();

            self.event_store
                .save_all(job.id().to_string().as_str(), &events)
                .map_err(|e| Error::RepositoryError(e.to_string()))?;

            self.job_repo
                .update(&job)
                .map_err(|e| Error::RepositoryError(e.to_string()))?;

            for event in &events {
                if let Err(e) = self
                    .event_bus
                    .publish(event.event_name(), &event.serialize_payload())
                {
                    tracing::warn!(
                        target = "sapo_printer::application::use_case::cancel_print_job",
                        event_type = %event.event_name(),
                        error = %e,
                        "Event bus publish failed (non-fatal)"
                    );
                }
            }

            cancelled += 1;
        }

        tracing::info!(
            target = "sapo_printer::application::use_case::cancel_print_job",
            slip_id = slip_id,
            cancelled = cancelled,
            "CancelPrintJobUseCase: completed"
        );

        Ok(cancelled)
    }
}
