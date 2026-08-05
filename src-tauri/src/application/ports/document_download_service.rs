use std::path::PathBuf;

use crate::domain::print_job::PrintJobId;
use crate::shared::errors::InfrastructureError;

pub trait DocumentDownloadService: Send + Sync {
    fn download(&self, url: &str, job_id: &PrintJobId) -> Result<PathBuf, InfrastructureError>;
}
