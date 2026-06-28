//! TempFileManager port — abstracts management of per-job temp file lifecycle.
//!
//! Use cases use this trait to wrap a downloaded path in an RAII guard and
//! to clean up orphaned temp files on cancel.

use std::path::{Path, PathBuf};

use crate::domain::print_job::PrintJobId;
use crate::shared::errors::InfrastructureError;

/// RAII handle to a temp file. Implementations are responsible for deleting
/// the underlying file when the handle is dropped (unless `keep` was called).
pub trait TempFileHandle: Send {
    fn path(&self) -> &Path;
    /// Mark the file to be retained after drop (e.g. on deferred failure cleanup).
    fn keep(&mut self);
}

pub trait TempFileManager: Send + Sync {
    /// Wrap `path` in an RAII handle whose Drop removes the file.
    ///
    /// Returns an error if `path` lies outside the managed temp directory.
    fn wrap(&self, path: PathBuf) -> Result<Box<dyn TempFileHandle>, InfrastructureError>;

    /// Best-effort cleanup of the temp file associated with `job_id`.
    /// Errors are logged by the implementation; this method should not fail.
    fn cleanup_for_job(&self, job_id: &PrintJobId);
}
