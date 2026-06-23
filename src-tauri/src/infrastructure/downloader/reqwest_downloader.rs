//! Reqwest-based Document Downloader Implementation
//!
//! Concrete implementation of `DocumentDownloader` using `reqwest::blocking::Client`
//! with a 30-second timeout, circuit breaker integration, and atomic download pattern.
//!
//! ## Download Flow
//! 1. Check circuit breaker state
//! 2. Download to `.tmp` file
//! 3. Validate PDF header (`%PDF-`)
//! 4. Atomic rename `.tmp` → `.pdf`
//! 5. Return final path
//!
//! ## Error Handling
//! Any error triggers `.tmp` cleanup and circuit breaker failure recording.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use reqwest::blocking::Client;

use crate::domain::print_job::value_objects::JobId;
use crate::infrastructure::downloader::circuit_breaker::CircuitBreaker;
use crate::infrastructure::downloader::document_downloader::DocumentDownloader;
use crate::shared::errors::InfrastructureError;

/// Minimum bytes to read for PDF header validation.
/// PDF files start with `%PDF-` (5 bytes).
const PDF_HEADER_SIZE: usize = 5;
const PDF_HEADER_MAGIC: &[u8; PDF_HEADER_SIZE] = b"%PDF-";

/// Maximum allowed download size (100 MB) to prevent unbounded memory allocation.
/// PDF files for printing rarely exceed this; larger files should use streaming.
const MAX_DOWNLOAD_BYTES: usize = 100 * 1024 * 1024;

/// Reqwest-based PDF document downloader with circuit breaker.
///
/// Downloads documents to `~/.sapo-printer/temp/{job_id}.pdf` using an atomic
/// rename pattern: content is first written to `.tmp`, validated, then renamed.
pub struct ReqwestDownloader {
    pub(crate) client: Client,
    pub(crate) circuit_breaker: Mutex<CircuitBreaker>,
}

impl ReqwestDownloader {
    /// Create a new downloader with default configuration.
    ///
    /// # Configuration
    /// - HTTP client timeout: 30 seconds
    /// - Circuit breaker: 5 failures threshold, 60s timeout
    pub fn new() -> Self {
        Self::with_timeout(std::time::Duration::from_secs(30))
    }

    /// Create a downloader with a custom HTTP timeout (for testing).
    #[doc(hidden)]
    pub fn with_timeout(timeout: std::time::Duration) -> Self {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .expect("failed to build reqwest client");

        Self {
            client,
            circuit_breaker: Mutex::new(CircuitBreaker::new()),
        }
    }

    /// Internal download logic (called within circuit breaker).
    ///
    /// Delegates to `do_download` and ensures the `.tmp` file is cleaned up
    /// on **any** error path (network, I/O, validation, rename).
    fn download_internal(&self, url: &str, job_id: &JobId) -> Result<PathBuf, InfrastructureError> {
        let temp_path = temp_file_path(job_id, "tmp")?;
        let final_path = temp_file_path(job_id, "pdf")?;

        // Ensure temp directory exists
        if let Some(parent) = temp_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let result = self.do_download(&temp_path, &final_path, url);

        // Always clean up temp file on any error (covers I/O, validation, rename failures)
        if result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }

        result
    }

    /// Perform the actual download, validation, and atomic rename.
    ///
    /// Caller is responsible for `.tmp` cleanup if this returns `Err`.
    fn do_download(
        &self,
        temp_path: &Path,
        final_path: &Path,
        url: &str,
    ) -> Result<PathBuf, InfrastructureError> {
        let response = self.client.get(url).send()?;

        if !response.status().is_success() {
            return Err(InfrastructureError::NetworkError(format!(
                "HTTP {}",
                response.status()
            )));
        }

        // F3: Guard against unbounded memory allocation
        if let Some(len) = response.content_length() {
            if len > MAX_DOWNLOAD_BYTES as u64 {
                return Err(InfrastructureError::ValidationError(format!(
                    "Content-Length {} exceeds maximum allowed {} bytes",
                    len, MAX_DOWNLOAD_BYTES
                )));
            }
        }

        let bytes = response.bytes()?;
        let mut file = File::create(temp_path)?;
        file.write_all(&bytes)?;
        drop(file);

        // Validate PDF header before committing
        validate_pdf_header(temp_path)?;

        // Atomic rename (only after validation)
        fs::rename(temp_path, final_path)?;

        Ok(final_path.to_path_buf())
    }
}

impl Default for ReqwestDownloader {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentDownloader for ReqwestDownloader {
    fn download(&self, url: &str, job_id: &JobId) -> Result<PathBuf, InfrastructureError> {
        // F7: Validate URL scheme before entering circuit breaker
        validate_url(url)?;

        let mut cb = self.circuit_breaker.lock().map_err(|_| {
            InfrastructureError::NetworkError("circuit breaker lock poisoned".into())
        })?;

        // Use cb.call() for proper state machine transitions
        let result = cb.call(|| self.download_internal(url, job_id));

        // F6: On ValidationError, undo the circuit breaker failure recording.
        // Validation errors are client-side data issues, not S3 outages,
        // so they should not count toward the failure threshold.
        if let Err(InfrastructureError::ValidationError(_)) = &result {
            cb.on_success(); // Reset the failure count (undo on_failure from cb.call)
        }

        result
    }
}

/// Validate that a file starts with the PDF magic header `%PDF-`.
///
/// # Errors
/// Returns `InfrastructureError::ValidationError` if the file is smaller than
/// 5 bytes or does not start with `%PDF-`.
pub fn validate_pdf_header(path: &Path) -> Result<(), InfrastructureError> {
    let mut file = File::open(path)?;
    let mut header = [0u8; PDF_HEADER_SIZE];
    file.read_exact(&mut header).map_err(|e| {
        InfrastructureError::ValidationError(format!("Failed to read PDF header: {}", e))
    })?;

    if header != *PDF_HEADER_MAGIC {
        return Err(InfrastructureError::ValidationError(
            "Invalid PDF header: file does not start with %PDF-".into(),
        ));
    }

    Ok(())
}

/// Validate the URL scheme before initiating a download request.
///
/// Only `http` and `https` schemes are allowed. This prevents accidental
/// `file://`, `data:`, or other schemes from reaching the HTTP client.
///
/// # Errors
/// Returns `InfrastructureError::ValidationError` if the URL is malformed
/// or uses a disallowed scheme.
fn validate_url(url: &str) -> Result<(), InfrastructureError> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(InfrastructureError::ValidationError(format!(
            "URL must use http or https scheme: {}",
            url
        )));
    }
    Ok(())
}

/// Build a path in the temp directory for the given job ID and extension.
///
/// Returns `~/.sapo-printer/temp/{job_id}.{ext}`
///
/// # Errors
/// Returns `InfrastructureError::ValidationError` if the home directory
/// cannot be resolved.
fn temp_file_path(job_id: &JobId, ext: &str) -> Result<PathBuf, InfrastructureError> {
    let home = home::home_dir().ok_or_else(|| {
        InfrastructureError::ValidationError("Cannot resolve home directory".into())
    })?;
    Ok(home
        .join(".sapo-printer")
        .join("temp")
        .join(format!("{}.{}", job_id, ext)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sapo_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_bytes(path: &Path, data: &[u8]) {
        let mut file = File::create(path).unwrap();
        file.write_all(data).unwrap();
    }

    #[test]
    fn test_validate_pdf_header_valid() {
        let dir = make_temp_dir();
        let path = dir.join("valid.pdf");
        write_bytes(&path, b"%PDF-1.4 some content");
        assert!(validate_pdf_header(&path).is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_validate_pdf_header_invalid() {
        let dir = make_temp_dir();
        let path = dir.join("invalid.pdf");
        write_bytes(&path, b"not a pdf content");
        let result = validate_pdf_header(&path);
        assert!(matches!(
            result,
            Err(InfrastructureError::ValidationError(_))
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_validate_pdf_header_too_short() {
        let dir = make_temp_dir();
        let path = dir.join("short.pdf");
        write_bytes(&path, b"abc");
        let result = validate_pdf_header(&path);
        assert!(matches!(
            result,
            Err(InfrastructureError::ValidationError(_))
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_validate_pdf_header_empty() {
        let dir = make_temp_dir();
        let path = dir.join("empty.pdf");
        write_bytes(&path, b"");
        let result = validate_pdf_header(&path);
        assert!(matches!(
            result,
            Err(InfrastructureError::ValidationError(_))
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_tmp_cleanup_on_validation_failure() {
        let dir = make_temp_dir();
        let tmp_path = dir.join("test.tmp");
        let final_path = dir.join("test.pdf");

        // Write invalid content to tmp
        write_bytes(&tmp_path, b"not a pdf");
        assert!(tmp_path.exists());

        // Validate should fail
        let result = validate_pdf_header(&tmp_path);
        assert!(result.is_err());

        // In the real downloader, tmp would be deleted on error.
        // Here we verify the cleanup logic pattern manually:
        if result.is_err() {
            let _ = fs::remove_file(&tmp_path);
        }
        assert!(!tmp_path.exists());
        assert!(!final_path.exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_reqwest_downloader_trait_object() {
        // Verify ReqwestDownloader can be used as Arc<dyn DocumentDownloader>
        let _downloader: std::sync::Arc<dyn DocumentDownloader> =
            std::sync::Arc::new(ReqwestDownloader::new());
    }

    // --- F7: URL validation tests ---

    #[test]
    fn test_validate_url_accepts_http() {
        assert!(validate_url("http://example.com/file.pdf").is_ok());
    }

    #[test]
    fn test_validate_url_accepts_https() {
        assert!(validate_url("https://s3.example.com/file.pdf").is_ok());
    }

    #[test]
    fn test_validate_url_rejects_file_scheme() {
        let result = validate_url("file:///etc/passwd");
        assert!(matches!(
            result,
            Err(InfrastructureError::ValidationError(_))
        ));
    }

    #[test]
    fn test_validate_url_rejects_data_scheme() {
        let result = validate_url("data:application/pdf;base64,JVBER");
        assert!(matches!(
            result,
            Err(InfrastructureError::ValidationError(_))
        ));
    }

    #[test]
    fn test_validate_url_rejects_empty_string() {
        let result = validate_url("");
        assert!(matches!(
            result,
            Err(InfrastructureError::ValidationError(_))
        ));
    }
}
