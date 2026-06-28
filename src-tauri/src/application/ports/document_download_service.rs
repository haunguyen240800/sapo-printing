//! DocumentDownloadService port — application abstraction over remote document fetching.
//!
//! Use cases depend on this trait; concrete implementations (e.g. `ReqwestDownloader`)
//! live in the infrastructure layer.

use std::path::PathBuf;

use crate::domain::print_job::PrintJobId;
use crate::shared::errors::InfrastructureError;

/// Service contract for downloading PDF documents from remote URLs.
///
/// Implementations must:
/// - Download the file to the temp directory
/// - Validate the PDF header (`%PDF-`)
/// - Use atomic rename (`.tmp` → `.pdf`) only after validation
/// - Clean up `.tmp` files on any error
pub trait DocumentDownloadService: Send + Sync {
    /// Download a PDF document from the given URL.
    ///
    /// # Arguments
    /// * `url` - The URL to download from (S3 presigned or public URL)
    /// * `job_id` - The print job identifier, used as the filename
    ///
    /// # Returns
    /// `PathBuf` pointing to the validated `.pdf` file in the temp directory.
    fn download(&self, url: &str, job_id: &PrintJobId) -> Result<PathBuf, InfrastructureError>;
}
