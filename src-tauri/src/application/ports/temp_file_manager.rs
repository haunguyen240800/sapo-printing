use std::path::{Path, PathBuf};

use crate::domain::print_job::PrintJobId;
use crate::shared::errors::InfrastructureError;

pub trait TempFileHandle: Send {
    fn path(&self) -> &Path;
    fn keep(&mut self);
}

pub trait TempFileManager: Send + Sync {
    fn wrap(&self, path: PathBuf) -> Result<Box<dyn TempFileHandle>, InfrastructureError>;

    fn cleanup_for_job(&self, job_id: &PrintJobId);
}
