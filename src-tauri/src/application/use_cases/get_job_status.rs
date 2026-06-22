use std::sync::Arc;

use crate::application::errors::Error;
use crate::application::models::PrintJobStatusResponse;
use crate::domain::print_job::{PrintJobId, PrintJobRepository};

pub struct GetJobStatusUseCase {
    job_repo: Arc<dyn PrintJobRepository>,
}

impl GetJobStatusUseCase {
    pub fn new(job_repo: Arc<dyn PrintJobRepository>) -> Self {
        Self { job_repo }
    }

    pub fn execute(&self, job_id: &str) -> Result<PrintJobStatusResponse, Error> {
        tracing::info!(
            target = "sapo_printer::application::use_case::get_job_status",
            job_id = job_id,
            "GetJobStatusUseCase: starting"
        );

        let job_id_parsed = job_id
            .parse::<PrintJobId>()
            .map_err(|_| Error::InvalidJobId {
                job_id: job_id.to_string(),
            })?;

        let job = self
            .job_repo
            .find_by_id(&job_id_parsed)
            .map_err(|e| Error::RepositoryError(format!("Failed to load job: {:?}", e)))?
            .ok_or_else(|| Error::JobNotFound {
                job_id: job_id.to_string(),
            })?;

        let dto = PrintJobStatusResponse::from(job);

        tracing::debug!(
            target = "sapo_printer::application::use_case::get_job_status",
            job_id = job_id,
            status = dto.status,
            "GetJobStatusUseCase: completed"
        );

        Ok(dto)
    }
}
