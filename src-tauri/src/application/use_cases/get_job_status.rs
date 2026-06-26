use std::sync::Arc;

use crate::application::dto::job_status_dto::JobStatusDto;
use crate::application::use_cases::errors::ApplicationError;
use crate::domain::print_job::PrintJobRepository;
use crate::domain::print_job::JobId;

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


