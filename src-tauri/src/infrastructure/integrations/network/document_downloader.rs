//! Document Downloader â€” Service Contract
//!
//! Defines the `DocumentDownloader` trait: a service contract for downloading
//! PDF documents from URLs (typically S3 presigned or public URLs) for bulk printing.
//!
//! ## Purpose
//! Download documents from remote URLs into the local temp directory
//! (`~/.sapo-printer/temp/`) so they can be processed by the print queue.
//!
//! ## Implementation Note
//! Concrete implementations MUST validate that downloaded content is a valid PDF
//! (check `%PDF-` header) before returning the final path.
//!
//! ## Integration
//! Used by `QueueWorker` (Story 3.5) to fetch documents before submitting them
//! to the print queue. Injected via `Arc<dyn DocumentDownloader>` in AppContext.

use std::path::PathBuf;

use crate::domain::models::JobId;
use crate::shared::errors::InfrastructureError;

/// Service contract for downloading PDF documents from remote URLs.
///
/// Implementations must:
/// - Download the file to the temp directory
/// - Validate the PDF header (`%PDF-`)
/// - Use atomic rename (`.tmp` â†’ `.pdf`) only after validation
/// - Clean up `.tmp` files on any error
///
/// # Thread Safety
/// `Send + Sync` bounds enable use with `Arc<dyn DocumentDownloader>` across
/// async tasks and thread pools.
pub trait DocumentDownloader: Send + Sync {
    /// Download a PDF document from the given URL.
    ///
    /// # Arguments
    /// * `url` - The URL to download from (S3 presigned or public URL)
    /// * `job_id` - The print job identifier, used as the filename
    ///
    /// # Returns
    /// `PathBuf` pointing to the validated `.pdf` file in the temp directory.
    ///
    /// # Errors
    /// - `InfrastructureError::NetworkError` â€” HTTP failure, DNS error, TLS error
    /// - `InfrastructureError::ValidationError` â€” downloaded content is not a valid PDF
    /// - `InfrastructureError::TimeoutError` â€” request exceeded 30s timeout
    /// - `InfrastructureError::CircuitOpenError` â€” circuit breaker is open, call rejected
    fn download(&self, url: &str, job_id: &JobId) -> Result<PathBuf, InfrastructureError>;
}
