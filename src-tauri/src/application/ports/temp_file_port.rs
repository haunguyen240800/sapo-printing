use std::path::{Path, PathBuf};

use crate::application::errors::Error;
use crate::domain::print_job::PrintJobId;

pub trait TempFileHandle: Send {
    fn path(&self) -> &Path;
    fn keep(&mut self);
}

pub trait TempFilePort: Send + Sync {
    fn wrap(&self, path: PathBuf) -> Result<Box<dyn TempFileHandle>, Error>;

    fn cleanup_for_job(&self, job_id: &PrintJobId);
}
