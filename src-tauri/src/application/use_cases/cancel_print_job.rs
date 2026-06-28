use std::sync::Arc;

use crate::application::dto::cancel_print_job_request::CancelPrintJobRequest;
use crate::application::ports::{EventStore, TempFileManager};
use crate::application::errors::ApplicationError;
use crate::domain::print_job::{PrintJobError, PrintJobId, PrintJobRepository};
use crate::shared::event_bus::EventBus;

/// Use case: Cancel a print job.
///
/// Business rules enforced:
/// - Job must exist (ApplicationError::JobNotFound)
/// - Job must be cancellable per domain rules
/// - Temp file cleanup via TempFileManager (best-effort)
pub struct CancelPrintJobUseCase {
    job_repo: Arc<dyn PrintJobRepository>,
    event_store: Arc<dyn EventStore>,
    event_bus: Arc<dyn EventBus>,
    temp_files: Arc<dyn TempFileManager>,
}

impl CancelPrintJobUseCase {
    pub fn new(
        job_repo: Arc<dyn PrintJobRepository>,
        event_store: Arc<dyn EventStore>,
        event_bus: Arc<dyn EventBus>,
        temp_files: Arc<dyn TempFileManager>,
    ) -> Self {
        Self {
            job_repo,
            event_store,
            event_bus,
            temp_files,
        }
    }

    pub fn execute(&self, request: CancelPrintJobRequest) -> Result<(), ApplicationError> {
        tracing::info!(
            target = "sapo_printer::application::use_case::cancel_print_job",
            job_id = request.job_id,
            "CancelPrintJobUseCase: starting"
        );

        let job_id =
            request
                .job_id
                .parse::<PrintJobId>()
                .map_err(|_| ApplicationError::InvalidJobId {
                    job_id: request.job_id.clone(),
                })?;

        let mut job = self
            .job_repo
            .find_by_id(&job_id)
            .map_err(|e| ApplicationError::RepositoryError(format!("Failed to load job: {:?}", e)))?
            .ok_or_else(|| ApplicationError::JobNotFound {
                job_id: job_id.to_string(),
            })?;

        job.cancel().map_err(|e| match e {
            PrintJobError::CannotCancelCompleted => {
                ApplicationError::CannotCancelCompleted {
                    job_id: job_id.to_string(),
                }
            }
            PrintJobError::CannotCancelFailed => {
                ApplicationError::CannotCancelFailed {
                    job_id: job_id.to_string(),
                }
            }
            PrintJobError::CannotCancelCancelled => {
                ApplicationError::CannotCancelCancelled {
                    job_id: job_id.to_string(),
                }
            }
            _ => ApplicationError::DomainRuleViolation {
                reason: format!("{:?}", e),
            },
        })?;

        let events = job.drain_events();

        self.job_repo.update(&job).map_err(|e| {
            ApplicationError::RepositoryError(format!("Failed to update job: {:?}", e))
        })?;

        self.event_store
            .save_all(job_id.to_string().as_str(), &events)
            .map_err(|e| ApplicationError::EventStoreError {
                reason: format!("Failed to save events: {:?}", e),
            })?;

        for event in &events {
            let payload = event.serialize_payload();
            if let Err(e) = self.event_bus.publish(event.event_name(), &payload) {
                tracing::warn!(
                    target = "sapo_printer::application::use_case::cancel_print_job",
                    event_type = event.event_name(),
                    error = %e,
                    "Failed to publish event (non-fatal)"
                );
            }
        }

        // Best-effort temp file cleanup via injected port
        self.temp_files.cleanup_for_job(&job_id);

        tracing::debug!(
            target = "sapo_printer::application::use_case::cancel_print_job",
            job_id = %job_id,
            "CancelPrintJobUseCase: completed"
        );

        Ok(())
    }
}
