use std::sync::Arc;

use crate::application::dto::cancel_job_request::CancelJobRequest;
use crate::application::use_cases::errors::ApplicationError;
use crate::domain::print_job::PrintJobRepository;
use crate::domain::print_job::JobId;
use crate::infrastructure::database::SqliteEventStore;
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
            crate::domain::print_job::errors::DomainError::CannotCancelCompleted => {
                ApplicationError::CannotCancelCompleted {
                    job_id: job_id.to_string(),
                }
            }
            crate::domain::print_job::errors::DomainError::CannotCancelFailed => {
                ApplicationError::CannotCancelFailed {
                    job_id: job_id.to_string(),
                }
            }
            crate::domain::print_job::errors::DomainError::CannotCancelCancelled => {
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

// â”€â”€ Unit Tests â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::print_job::PrintJob;
    use crate::domain::print_job::PrintStatus;
    use crate::infrastructure::database::run_migrations;
    use crate::infrastructure::secrets::SecretManager;
    use crate::shared::errors::InfrastructureError;
    use crate::shared::event_bus::InMemoryEventBus;
    use rusqlite::Connection;
    use std::collections::HashMap;
    use std::sync::Mutex as StdMutex;

    struct MockSecretManager {
        store: StdMutex<HashMap<String, String>>,
    }

    impl MockSecretManager {
        fn new() -> Self {
            Self {
                store: StdMutex::new(HashMap::new()),
            }
        }
    }

    impl SecretManager for MockSecretManager {
        fn store(&self, key: &str, value: &str) -> Result<(), InfrastructureError> {
            self.store
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_string());
            Ok(())
        }

        fn retrieve(&self, key: &str) -> Result<Option<String>, InfrastructureError> {
            Ok(self.store.lock().unwrap().get(key).cloned())
        }

        fn delete(&self, key: &str) -> Result<(), InfrastructureError> {
            self.store.lock().unwrap().remove(key);
            Ok(())
        }
    }

    struct MockPrintJobRepository {
        jobs: StdMutex<Vec<PrintJob>>,
    }

    impl MockPrintJobRepository {
        fn new() -> Self {
            Self {
                jobs: StdMutex::new(Vec::new()),
            }
        }

        fn add_job(&self, job: PrintJob) {
            self.jobs.lock().unwrap().push(job);
        }
    }

    impl PrintJobRepository for MockPrintJobRepository {
        fn save(
            &self,
            job: &PrintJob,
        ) -> Result<(), crate::domain::print_job::errors::DomainError> {
            self.jobs.lock().unwrap().push(job.clone());
            Ok(())
        }

        fn update(
            &self,
            job: &PrintJob,
        ) -> Result<(), crate::domain::print_job::errors::DomainError> {
            let mut jobs = self.jobs.lock().unwrap();
            if let Some(pos) = jobs.iter().position(|j| j.id() == job.id()) {
                jobs[pos] = job.clone();
                Ok(())
            } else {
                Err(
                    crate::domain::print_job::errors::DomainError::RepositoryError {
                        reason: "Job not found".to_string(),
                    },
                )
            }
        }

        fn find_by_id(
            &self,
            id: &JobId,
        ) -> Result<Option<PrintJob>, crate::domain::print_job::errors::DomainError> {
            Ok(self
                .jobs
                .lock()
                .unwrap()
                .iter()
                .find(|j| j.id() == id)
                .cloned())
        }

        fn find_by_status(
            &self,
            status: &PrintStatus,
        ) -> Result<Vec<PrintJob>, crate::domain::print_job::errors::DomainError> {
            Ok(self
                .jobs
                .lock()
                .unwrap()
                .iter()
                .filter(|j| j.status() == status)
                .cloned()
                .collect())
        }

        fn find_all(&self) -> Result<Vec<PrintJob>, crate::domain::print_job::errors::DomainError> {
            Ok(self.jobs.lock().unwrap().clone())
        }
    }

    fn setup_test_infrastructure() -> (
        Arc<MockPrintJobRepository>,
        Arc<SqliteEventStore>,
        Arc<InMemoryEventBus>,
    ) {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();
        let arc_conn = Arc::new(StdMutex::new(conn));

        let job_repo = Arc::new(MockPrintJobRepository::new());
        let event_store = Arc::new(SqliteEventStore::new(arc_conn, Arc::new(MockSecretManager::new())));
        let event_bus = Arc::new(InMemoryEventBus::new());

        (job_repo, event_store, event_bus)
    }

    #[test]
    fn test_cancel_pending_job_succeeds() {
        let (job_repo, event_store, event_bus) = setup_test_infrastructure();

        // Create job in PENDING state
        let job = PrintJob::new("https://example.com/doc.pdf".to_string(), "HP".to_string());
        let job_id = job.id().to_string();
        job_repo.add_job(job);

        // Cancel job
        let use_case = CancelPrintJobUseCase::new(job_repo.clone(), event_store, event_bus);
        let request = CancelJobRequest {
            job_id: job_id.clone(),
        };
        let result = use_case.execute(request);

        assert!(result.is_ok());

        // Verify job status is CANCELLED
        let updated_job = job_repo
            .find_by_id(&job_id.parse().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(*updated_job.status(), PrintStatus::Cancelled);
    }

    #[test]
    fn test_cancel_queued_job_succeeds() {
        let (job_repo, event_store, event_bus) = setup_test_infrastructure();

        // Create job in QUEUED state
        let mut job = PrintJob::new("https://example.com/doc.pdf".to_string(), "HP".to_string());
        job.queue().unwrap();
        let job_id = job.id().to_string();
        job_repo.add_job(job);

        // Cancel job
        let use_case = CancelPrintJobUseCase::new(job_repo.clone(), event_store, event_bus);
        let request = CancelJobRequest {
            job_id: job_id.clone(),
        };
        let result = use_case.execute(request);

        assert!(result.is_ok());

        // Verify job status is CANCELLED
        let updated_job = job_repo
            .find_by_id(&job_id.parse().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(*updated_job.status(), PrintStatus::Cancelled);
    }

    #[test]
    fn test_cannot_cancel_completed_job() {
        let (job_repo, event_store, event_bus) = setup_test_infrastructure();

        // Create completed job
        let mut job = PrintJob::new("https://example.com/doc.pdf".to_string(), "HP".to_string());
        job.queue().unwrap();
        job.mark_downloaded().unwrap();
        job.mark_submitted().unwrap();
        job.mark_printing().unwrap();
        job.complete().unwrap();
        let job_id = job.id().to_string();
        job_repo.add_job(job);

        // Attempt cancel
        let use_case = CancelPrintJobUseCase::new(job_repo, event_store, event_bus);
        let request = CancelJobRequest {
            job_id: job_id.clone(),
        };
        let result = use_case.execute(request);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApplicationError::CannotCancelCompleted { .. }
        ));
    }

    #[test]
    fn test_cannot_cancel_failed_job() {
        let (job_repo, event_store, event_bus) = setup_test_infrastructure();

        // Create failed job
        let mut job = PrintJob::new("https://example.com/doc.pdf".to_string(), "HP".to_string());
        job.queue().unwrap();
        job.fail("Test error".to_string()).unwrap();
        let job_id = job.id().to_string();
        job_repo.add_job(job);

        // Attempt cancel
        let use_case = CancelPrintJobUseCase::new(job_repo, event_store, event_bus);
        let result = use_case.execute(CancelJobRequest { job_id });

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApplicationError::CannotCancelFailed { .. }
        ));
    }

    #[test]
    fn test_cancel_printing_job_succeeds() {
        let (job_repo, event_store, event_bus) = setup_test_infrastructure();

        // Create job in PRINTING state
        let mut job = PrintJob::new("https://example.com/doc.pdf".to_string(), "HP".to_string());
        job.queue().unwrap();
        job.mark_downloaded().unwrap();
        job.mark_submitted().unwrap();
        job.mark_printing().unwrap();
        let job_id = job.id().to_string();
        job_repo.add_job(job);

        // Cancel job
        let use_case = CancelPrintJobUseCase::new(job_repo.clone(), event_store, event_bus);
        let result = use_case.execute(CancelJobRequest {
            job_id: job_id.clone(),
        });

        // Should succeed (domain allows cancelling PRINTING jobs)
        assert!(result.is_ok());

        // Verify status is CANCELLED
        let updated_job = job_repo
            .find_by_id(&job_id.parse().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(*updated_job.status(), PrintStatus::Cancelled);
    }

    #[test]
    fn test_cancel_nonexistent_job_fails() {
        let (job_repo, event_store, event_bus) = setup_test_infrastructure();

        let use_case = CancelPrintJobUseCase::new(job_repo, event_store, event_bus);
        let result = use_case.execute(CancelJobRequest {
            job_id: "00000000-0000-0000-0000-000000000000".to_string(),
        });

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApplicationError::JobNotFound { .. }
        ));
    }

    #[test]
    fn test_cancel_with_invalid_job_id_fails() {
        let (job_repo, event_store, event_bus) = setup_test_infrastructure();

        let use_case = CancelPrintJobUseCase::new(job_repo, event_store, event_bus);
        let result = use_case.execute(CancelJobRequest {
            job_id: "invalid-uuid".to_string(),
        });

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApplicationError::InvalidJobId { .. }
        ));
    }
}
