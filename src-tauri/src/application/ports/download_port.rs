use std::path::PathBuf;

use crate::application::errors::Error;
use crate::domain::print_job::PrintJobId;

pub trait DownloadPort: Send + Sync {
    fn download(&self, url: &str, job_id: &PrintJobId) -> Result<PathBuf, Error>;
}
