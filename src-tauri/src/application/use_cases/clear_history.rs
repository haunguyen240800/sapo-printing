use std::sync::Arc;

use crate::application::errors::Error;
use crate::domain::print_job::PrintJobRepository;

pub struct ClearHistoryUseCase {
    job_repo: Arc<dyn PrintJobRepository>,
}

impl ClearHistoryUseCase {
    pub fn new(job_repo: Arc<dyn PrintJobRepository>) -> Self {
        Self { job_repo }
    }

    pub fn execute(&self) -> Result<u64, Error> {
        tracing::info!(
            target = "sapo_printer::application::use_case::clear_history",
            "ClearHistoryUseCase: starting"
        );

        let deleted = self.job_repo.clear_terminal()?;

        tracing::info!(
            target = "sapo_printer::application::use_case::clear_history",
            deleted_jobs = deleted,
            "ClearHistoryUseCase: completed"
        );

        Ok(deleted)
    }
}
