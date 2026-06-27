use std::sync::Arc;

use crate::application::dto::cancel_job_request::CancelJobRequest;
use crate::application::use_cases::errors::ApplicationError;
use crate::domain::repository::PrintJobRepository;
use crate::domain::models::JobId;
use crate::infrastructure::persistence::sqlite::SqliteEventStore;
use crate::shared::event_bus::EventBus;

/// Use case: Cancel a print job.
///
/// Business rules enforced:
/// - Job must exist (ApplicationError::JobNotFound)
/// - Job must be cancellable per domain rules (cannot cancel COMPLETED, FAILED, CANCELLED)
/// - Temp file cleanup after cancellation (best-effort)
///
/// Transaction boundary:
/// 1. Load job from repository
/// 2. Call job.cancel() (domain validation)
/// 3. Update repository
/// 4. Save events to event store
/// 5. Publish events to event bus (after commit)
pub struct CancelPrintJobUseCase {
    job_repo: Arc<dyn PrintJobRepository>,
    event_store: Arc<SqliteEventStore>,
    event_bus: Arc<dyn EventBus>,
}

impl CancelPrintJobUseCase {
    pub fn new(
        job_repo: Arc<dyn PrintJobRepository>,
        event_store: Arc<SqliteEventStore>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            job_repo,
            event_store,
            event_bus,
        }
    }

    pub fn execute(&self, request: CancelJobRequest) -> Result<(), ApplicationError> {
        tracing::info!(
            target = "sapo_printer::use_case::cancel_print_job",
            job_id = request.job_id,
            "CancelPrintJobUseCase: starting"
        );

        // Parse JobId
        let job_id =
            request
                .job_id
                .parse::<JobId>()
                .map_err(|_| ApplicationError::InvalidJobId {
                    job_id: request.job_id.clone(),
                })?;

        // Load job
        let mut job = self
            .job_repo
            .find_by_id(&job_id)
            .map_err(|e| ApplicationError::RepositoryError(format!("Failed to load job: {:?}", e)))?
            .ok_or_else(|| ApplicationError::JobNotFound {
                job_id: job_id.to_string(),
            })?;

        // Call domain cancel (validates business rules)
        job.cancel().map_err(|e| match e {
            crate::domain::models::DomainError::CannotCancelCompleted => {
                ApplicationError::CannotCancelCompleted {
                    job_id: job_id.to_string(),
                }
            }
            crate::domain::models::DomainError::CannotCancelFailed => {
                ApplicationError::CannotCancelFailed {
                    job_id: job_id.to_string(),
                }
            }
            crate::domain::models::DomainError::CannotCancelCancelled => {
                ApplicationError::CannotCancelCancelled {
                    job_id: job_id.to_string(),
                }
            }
            _ => ApplicationError::DomainRuleViolation {
                reason: format!("{:?}", e),
            },
        })?;

        // Collect events before update
        let events = job.drain_events();

        // Update job in repository (transaction)
        self.job_repo.update(&job).map_err(|e| {
            ApplicationError::RepositoryError(format!("Failed to update job: {:?}", e))
        })?;

        // Save events to event store (same transaction)
        self.event_store
            .save_all(job_id.to_string().as_str(), &events)
            .map_err(|e| ApplicationError::EventStoreError {
                reason: format!("Failed to save events: {:?}", e),
            })?;

        // Publish events (after commit)
        for event in &events {
            let payload = event.serialize_payload();
            if let Err(e) = self.event_bus.publish(event.event_name(), &payload) {
                eprintln!(
                    "Warning: Failed to publish event {}: {}",
                    event.event_name(),
                    e
                );
            }
        }

        // Cleanup temp file (best-effort, don't fail if cleanup fails)
        self.cleanup_temp_file(&job_id);

        tracing::debug!(
            target = "sapo_printer::use_case::cancel_print_job",
            job_id = %job_id,
            "CancelPrintJobUseCase: completed"
        );

        Ok(())
    }

    fn cleanup_temp_file(&self, job_id: &JobId) {
        let temp_path = std::path::PathBuf::from(format!(
            "{}/.sapo-printer/temp/{}.pdf",
            std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_else(|_| ".".to_string()),
            job_id
        ));

        if temp_path.exists() {
            if let Err(e) = std::fs::remove_file(&temp_path) {
                eprintln!(
                    "Warning: Failed to cleanup temp file {:?}: {}",
                    temp_path, e
                );
            } else {
                tracing::info!("Cleaned up temp file: {:?}", temp_path);
            }
        }
    }
}

// ── Unit Tests ──────────────────────────────────────────────────────────────


