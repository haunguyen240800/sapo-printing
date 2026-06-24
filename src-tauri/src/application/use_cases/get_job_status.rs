use std::sync::Arc;

use crate::application::dto::job_status_dto::JobStatusDto;
use crate::application::use_cases::errors::ApplicationError;
use crate::domain::print_job::repository::PrintJobRepository;
use crate::domain::print_job::value_objects::JobId;

/// Use case: Get status of a print job.
///
/// Read-only operation — no transaction, no event publishing.
/// Returns JobStatusDto with progress calculation for web app polling.
pub struct GetJobStatusUseCase {
    job_repo: Arc<dyn PrintJobRepository>,
}

impl GetJobStatusUseCase {
    pub fn new(job_repo: Arc<dyn PrintJobRepository>) -> Self {
        Self { job_repo }
    }

    /// Execute the use case: look up a job by ID and return its status DTO.
    ///
    /// Returns:
    /// - `Ok(JobStatusDto)` if job exists
    /// - `Err(ApplicationError::JobNotFound)` if job doesn't exist
    /// - `Err(ApplicationError::InvalidJobId)` if job_id is not a valid UUID
    /// - `Err(ApplicationError::RepositoryError)` on storage failure
    pub fn execute(&self, job_id: &str) -> Result<JobStatusDto, ApplicationError> {
        tracing::info!(
            target = "sapo_printer::use_case::get_job_status",
            job_id = job_id,
            "GetJobStatusUseCase: starting"
        );

        // Parse JobId
        let job_id_parsed = job_id.parse::<JobId>().map_err(|_| ApplicationError::InvalidJobId {
            job_id: job_id.to_string(),
        })?;

        // Load job
        let job = self
            .job_repo
            .find_by_id(&job_id_parsed)
            .map_err(|e| ApplicationError::RepositoryError(format!("Failed to load job: {:?}", e)))?
            .ok_or_else(|| ApplicationError::JobNotFound {
                job_id: job_id.to_string(),
            })?;

        // Convert to DTO
        let dto = JobStatusDto::from(job);

        tracing::debug!(
            target = "sapo_printer::use_case::get_job_status",
            job_id = job_id,
            status = dto.status,
            "GetJobStatusUseCase: completed"
        );

        Ok(dto)
    }
}

// ── Unit Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::print_job::aggregate::PrintJob;
    use crate::domain::print_job::errors::DomainError as JobDomainError;
    use crate::domain::print_job::value_objects::PrintStatus;
    use std::sync::Mutex as StdMutex;

    struct MockJobRepo {
        jobs: StdMutex<Vec<PrintJob>>,
    }

    impl MockJobRepo {
        fn new() -> Self {
            Self { jobs: StdMutex::new(Vec::new()) }
        }

        fn add_job(&self, job: PrintJob) {
            self.jobs.lock().unwrap().push(job);
        }
    }

    impl PrintJobRepository for MockJobRepo {
        fn save(&self, job: &PrintJob) -> Result<(), JobDomainError> {
            self.jobs.lock().unwrap().push(job.clone());
            Ok(())
        }

        fn update(&self, job: &PrintJob) -> Result<(), JobDomainError> {
            let mut jobs = self.jobs.lock().unwrap();
            if let Some(pos) = jobs.iter().position(|j| j.id() == job.id()) {
                jobs[pos] = job.clone();
                Ok(())
            } else {
                Err(JobDomainError::RepositoryError {
                    reason: "not found".into(),
                })
            }
        }

        fn find_by_id(&self, id: &JobId) -> Result<Option<PrintJob>, JobDomainError> {
            Ok(self
                .jobs
                .lock()
                .unwrap()
                .iter()
                .find(|j| j.id() == id)
                .cloned())
        }

        fn find_by_status(&self, _status: &PrintStatus) -> Result<Vec<PrintJob>, JobDomainError> {
            Ok(self.jobs.lock().unwrap().clone())
        }

        fn find_all(&self) -> Result<Vec<PrintJob>, JobDomainError> {
            Ok(self.jobs.lock().unwrap().clone())
        }
    }

    #[test]
    fn test_execute_valid_job_returns_dto() {
        let repo = Arc::new(MockJobRepo::new());
        let job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP_LaserJet".to_string(),
        );
        let job_id = job.id().to_string();
        repo.add_job(job);

        let use_case = GetJobStatusUseCase::new(repo);
        let result = use_case.execute(&job_id);

        assert!(result.is_ok());
        let dto = result.unwrap();
        assert_eq!(dto.job_id, job_id);
        assert_eq!(dto.printer_name, "HP_LaserJet");
        assert_eq!(dto.status, "PENDING");
        assert_eq!(dto.progress, 0);
    }

    #[test]
    fn test_execute_nonexistent_job_returns_not_found() {
        let repo = Arc::new(MockJobRepo::new());
        let use_case = GetJobStatusUseCase::new(repo);

        let result = use_case.execute("00000000-0000-0000-0000-000000000000");

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ApplicationError::JobNotFound { .. }));
    }

    #[test]
    fn test_execute_invalid_job_id_returns_validation_error() {
        let repo = Arc::new(MockJobRepo::new());
        let use_case = GetJobStatusUseCase::new(repo);

        let result = use_case.execute("not-a-valid-uuid");

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApplicationError::InvalidJobId { .. }
        ));
    }

    #[test]
    fn test_execute_progress_per_state() {
        let repo = Arc::new(MockJobRepo::new());

        let cases = [
            (PrintStatus::Pending, 0),
            (PrintStatus::Queued, 10),
            (PrintStatus::Downloaded, 40),
            (PrintStatus::SubmittedToQueue, 60),
            (PrintStatus::Printing, 80),
            (PrintStatus::Completed, 100),
            (PrintStatus::Failed, 0),
            (PrintStatus::Cancelled, 0),
        ];

        for (status, expected_progress) in cases {
            let mut job = PrintJob::new(
                "https://s3.example.com/doc.pdf".to_string(),
                "HP_LaserJet".to_string(),
            );

            // Transition to desired status
            match status {
                PrintStatus::Pending => {} // already pending
                PrintStatus::Queued => { job.queue().unwrap(); }
                PrintStatus::Downloaded => {
                    job.queue().unwrap();
                    job.mark_downloaded().unwrap();
                }
                PrintStatus::SubmittedToQueue => {
                    job.queue().unwrap();
                    job.mark_downloaded().unwrap();
                    job.mark_submitted().unwrap();
                }
                PrintStatus::Printing => {
                    job.queue().unwrap();
                    job.mark_downloaded().unwrap();
                    job.mark_submitted().unwrap();
                    job.mark_printing().unwrap();
                }
                PrintStatus::Completed => {
                    job.queue().unwrap();
                    job.mark_downloaded().unwrap();
                    job.mark_submitted().unwrap();
                    job.mark_printing().unwrap();
                    job.complete().unwrap();
                }
                PrintStatus::Failed => {
                    job.queue().unwrap();
                    job.fail("test error".to_string()).unwrap();
                }
                PrintStatus::Cancelled => {
                    job.cancel().unwrap();
                }
            }

            let job_id = job.id().to_string();
            repo.add_job(job);

            let use_case = GetJobStatusUseCase::new(repo.clone());
            let dto = use_case.execute(&job_id).unwrap();

            assert_eq!(
                dto.progress, expected_progress,
                "Progress mismatch for {:?}: expected {}, got {}",
                status, expected_progress, dto.progress
            );
        }
    }

    #[test]
    fn test_execute_job_status_fields_populated() {
        let repo = Arc::new(MockJobRepo::new());
        let mut job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP_LaserJet".to_string(),
        );
        job.queue().unwrap();
        job.fail("Download timeout".to_string()).unwrap();
        let job_id = job.id().to_string();
        let created_at = job.created_at();
        repo.add_job(job);

        let use_case = GetJobStatusUseCase::new(repo);
        let dto = use_case.execute(&job_id).unwrap();

        assert_eq!(dto.status, "FAILED");
        assert_eq!(dto.progress, 0);
        assert_eq!(dto.printer_name, "HP_LaserJet");
        assert_eq!(dto.created_at, created_at);
        assert_eq!(dto.error_message, Some("Download timeout".to_string()));
    }
}
