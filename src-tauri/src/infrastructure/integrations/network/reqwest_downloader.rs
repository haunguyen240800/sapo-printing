use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use reqwest::blocking::Client;

use crate::application::ports::DocumentDownloadService;
use crate::domain::print_job::PrintJobId;
use crate::infrastructure::integrations::network::circuit_breaker::CircuitBreaker;
use crate::shared::errors::InfrastructureError;

const PDF_HEADER_SIZE: usize = 5;
const PDF_HEADER_MAGIC: &[u8; PDF_HEADER_SIZE] = b"%PDF-";

const MAX_DOWNLOAD_BYTES: usize = 100 * 1024 * 1024;

pub struct ReqwestDownloader {
    pub(crate) client: Client,
    pub(crate) circuit_breaker: Mutex<CircuitBreaker>,
}

impl ReqwestDownloader {
    pub fn new() -> Self {
        Self::with_timeout(std::time::Duration::from_secs(30))
    }

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

    fn download_internal(&self, url: &str, job_id: &PrintJobId) -> Result<PathBuf, InfrastructureError> {
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

    fn do_download(
        &self,
        temp_path: &Path,
        final_path: &Path,
        url: &str,
    ) -> Result<PathBuf, InfrastructureError> {
        tracing::info!(
            target = "sapo_printer::downloader",
            url = url,
            "ReqwestDownloader: download started"
        );

        let response = self.client.get(url).send()?;

        if !response.status().is_success() {
            let status = response.status();
            tracing::warn!(
                target = "sapo_printer::downloader",
                url = url,
                status = %status,
                "ReqwestDownloader: download failed"
            );
            // 4xx errors are client-side problems (bad URL, auth, missing file).
            // Return ValidationError so the circuit breaker does NOT count them —
            // these are not service outages and should not open the circuit.
            // 5xx errors ARE server problems and should count toward the threshold.
            if status.is_client_error() {
                return Err(InfrastructureError::ValidationError(format!(
                    "HTTP {} — check PDF URL or permissions",
                    status
                )));
            }
            return Err(InfrastructureError::NetworkError(format!(
                "HTTP {} — server error",
                status
            )));
        }

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

        validate_pdf_header(temp_path)?;

        fs::rename(temp_path, final_path)?;

        tracing::info!(
            target = "sapo_printer::downloader",
            url = url,
            "ReqwestDownloader: download completed"
        );

        Ok(final_path.to_path_buf())
    }
}

impl ReqwestDownloader {
    pub fn reset_circuit_breaker(&self) {
        if let Ok(mut cb) = self.circuit_breaker.lock() {
            cb.reset();
            tracing::info!(
                target = "sapo_printer::downloader",
                "Circuit breaker manually reset to Closed"
            );
        }
    }
}

impl Default for ReqwestDownloader {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentDownloadService for ReqwestDownloader {
    fn download(&self, url: &str, job_id: &PrintJobId) -> Result<PathBuf, InfrastructureError> {
        validate_url(url)?;

        let mut cb = self.circuit_breaker.lock().map_err(|_| {
            InfrastructureError::NetworkError("circuit breaker lock poisoned".into())
        })?;

        let result = cb.call(|| self.download_internal(url, job_id));

        if let Err(InfrastructureError::ValidationError(_)) = &result {
            cb.on_success();
        }

        result
    }
}

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

fn validate_url(url: &str) -> Result<(), InfrastructureError> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(InfrastructureError::ValidationError(format!(
            "URL must use http or https scheme: {}",
            url
        )));
    }
    Ok(())
}

fn temp_file_path(job_id: &PrintJobId, ext: &str) -> Result<PathBuf, InfrastructureError> {
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

        write_bytes(&tmp_path, b"not a pdf");
        assert!(tmp_path.exists());

        let result = validate_pdf_header(&tmp_path);
        assert!(result.is_err());

        if result.is_err() {
            let _ = fs::remove_file(&tmp_path);
        }
        assert!(!tmp_path.exists());
        assert!(!final_path.exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_reqwest_downloader_trait_object() {
        let _downloader: std::sync::Arc<dyn DocumentDownloadService> =
            std::sync::Arc::new(ReqwestDownloader::new());
    }

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
