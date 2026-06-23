//! Integration tests for the document downloader with circuit breaker.
//!
//! Uses `mockito` to simulate an HTTP server and test the full download flow:
//! HTTP response → file write → PDF validation → atomic rename → circuit breaker.

use std::fs;
use std::time::Duration;

use sapo_printer::domain::print_job::value_objects::JobId;
use sapo_printer::infrastructure::downloader::circuit_breaker::{CircuitBreaker, CircuitState};
use sapo_printer::infrastructure::downloader::{DocumentDownloader, ReqwestDownloader};
use sapo_printer::shared::errors::InfrastructureError;

/// A valid PDF magic header followed by dummy content.
fn fake_pdf_bytes() -> Vec<u8> {
    let mut data = b"%PDF-1.4 test content".to_vec();
    data.resize(1024, 0); // Pad to realistic size
    data
}

fn non_pdf_bytes() -> Vec<u8> {
    b"<html><body>Not a PDF</body></html>".to_vec()
}

/// Ensure temp directory exists and return its path.
fn ensure_temp_dir() -> std::path::PathBuf {
    let home = home::home_dir().unwrap();
    let dir = home.join(".sapo-printer").join("temp");
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Clean up any leftover test files for a given job_id.
fn cleanup_job_files(job_id: &JobId) {
    let temp_dir = ensure_temp_dir();
    let _ = fs::remove_file(temp_dir.join(format!("{}.tmp", job_id)));
    let _ = fs::remove_file(temp_dir.join(format!("{}.pdf", job_id)));
}

// ===== Test 1: Successful PDF download =====

#[test]
fn test_successful_pdf_download() {
    let mut server = mockito::Server::new();
    let _m = server
        .mock("GET", "/test.pdf")
        .with_status(200)
        .with_header("content-type", "application/pdf")
        .with_body(fake_pdf_bytes())
        .create();

    let downloader = ReqwestDownloader::new();
    let job_id = JobId::new();
    cleanup_job_files(&job_id);

    let url = format!("{}/test.pdf", server.url());
    let result = downloader.download(&url, &job_id);

    assert!(result.is_ok(), "download should succeed: {:?}", result);
    let path = result.unwrap();
    assert!(path.exists(), "PDF file should exist");
    assert!(
        path.to_string_lossy().ends_with(".pdf"),
        "path should end with .pdf"
    );

    cleanup_job_files(&job_id);
}

// ===== Test 2: Invalid content returns ValidationError =====

#[test]
fn test_invalid_content_returns_validation_error() {
    let mut server = mockito::Server::new();
    let _m = server
        .mock("GET", "/invalid.pdf")
        .with_status(200)
        .with_body(non_pdf_bytes())
        .create();

    let downloader = ReqwestDownloader::new();
    let job_id = JobId::new();
    cleanup_job_files(&job_id);

    let url = format!("{}/invalid.pdf", server.url());
    let result = downloader.download(&url, &job_id);

    assert!(
        matches!(result, Err(InfrastructureError::ValidationError(_))),
        "should return ValidationError, got: {:?}",
        result
    );

    // Verify .tmp file was cleaned up
    let temp_dir = ensure_temp_dir();
    assert!(
        !temp_dir.join(format!("{}.tmp", job_id)).exists(),
        ".tmp file should be cleaned up"
    );

    cleanup_job_files(&job_id);
}

// ===== Test 3: HTTP error returns NetworkError =====

#[test]
fn test_http_error_returns_network_error() {
    let mut server = mockito::Server::new();
    let _m = server.mock("GET", "/missing.pdf").with_status(404).create();

    let downloader = ReqwestDownloader::new();
    let job_id = JobId::new();
    cleanup_job_files(&job_id);

    let url = format!("{}/missing.pdf", server.url());
    let result = downloader.download(&url, &job_id);

    assert!(
        matches!(result, Err(InfrastructureError::NetworkError(_))),
        "should return NetworkError for HTTP 404, got: {:?}",
        result
    );

    cleanup_job_files(&job_id);
}

// ===== Test 4: Circuit breaker opens after 5 failures =====

#[test]
fn test_circuit_breaker_opens_after_failures() {
    let mut server = mockito::Server::new();
    let _m = server
        .mock("GET", mockito::Matcher::Any)
        .with_status(500)
        .create();

    let downloader = ReqwestDownloader::new();
    let temp_dir = ensure_temp_dir();

    // 5 failures should open the circuit
    for i in 0..5 {
        let job_id = JobId::new();
        let url = format!("{}/fail_{}.pdf", server.url(), i);
        let result = downloader.download(&url, &job_id);
        assert!(result.is_err(), "call {} should fail", i);
    }

    // 6th call should return CircuitOpenError immediately
    let job_id = JobId::new();
    let url = format!("{}/sixth.pdf", server.url());
    let result = downloader.download(&url, &job_id);

    assert!(
        matches!(result, Err(InfrastructureError::CircuitOpenError)),
        "6th call should return CircuitOpenError, got: {:?}",
        result
    );

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

// ===== Test 5: Circuit breaker state transitions (unit-level) =====
// Tests the circuit breaker directly rather than through the downloader,
// since the full recovery test would require waiting 60s for the timeout.

#[test]
fn test_circuit_breaker_state_transitions() {
    let mut cb = CircuitBreaker::new();

    // Closed state: initial
    assert_eq!(cb.state(), CircuitState::Closed);

    // 5 failures → Open
    for _ in 0..5 {
        let _ = cb.call(|| Err::<(), _>(InfrastructureError::NetworkError("fail".into())));
    }
    assert_eq!(cb.state(), CircuitState::Open);

    // Verify that calls are rejected while Open
    let result = cb.call(|| Ok::<_, InfrastructureError>(()));
    assert!(matches!(result, Err(InfrastructureError::CircuitOpenError)));

    // Verify failure count stays at 5 (rejected call doesn't increment)
    assert_eq!(cb.failure_count(), 5);

    // Use a fresh circuit for a success-after-recovery scenario
    let mut cb2 = CircuitBreaker::new();

    // Success in Closed state resets failure count
    let _ = cb2.call(|| Ok::<_, InfrastructureError>(()));
    assert_eq!(cb2.failure_count(), 0);
    assert_eq!(cb2.state(), CircuitState::Closed);

    // 4 failures (below threshold)
    for _ in 0..4 {
        let _ = cb2.call(|| Err::<(), _>(InfrastructureError::NetworkError("fail".into())));
    }
    assert_eq!(cb2.state(), CircuitState::Closed);
    assert_eq!(cb2.failure_count(), 4);

    // 1 more success resets
    let _ = cb2.call(|| Ok::<_, InfrastructureError>(()));
    assert_eq!(cb2.failure_count(), 0);
    assert_eq!(cb2.state(), CircuitState::Closed);
}

// ===== Test 6: End-to-end with valid PDF through full flow =====

#[test]
fn test_end_to_end_download_validate_rename() {
    let mut server = mockito::Server::new();

    // Large PDF-like content
    let mut pdf_bytes = b"%PDF-1.7".to_vec();
    pdf_bytes.resize(50_000, 0); // 50KB

    let _m = server
        .mock("GET", "/large.pdf")
        .with_status(200)
        .with_body(pdf_bytes.clone())
        .create();

    let downloader = ReqwestDownloader::new();
    let job_id = JobId::new();
    cleanup_job_files(&job_id);

    let url = format!("{}/large.pdf", server.url());
    let result = downloader.download(&url, &job_id);

    assert!(
        result.is_ok(),
        "large PDF download should succeed: {:?}",
        result
    );
    let path = result.unwrap();

    // Verify file exists and has correct content
    assert!(path.exists());
    let content = fs::read(&path).unwrap();
    assert!(content.starts_with(b"%PDF-"));

    // Verify .tmp does not exist (clean rename)
    let temp_dir = ensure_temp_dir();
    assert!(
        !temp_dir.join(format!("{}.tmp", job_id)).exists(),
        ".tmp file should not exist after successful rename"
    );

    cleanup_job_files(&job_id);
}

// ===== Test 7: Timeout returns TimeoutError =====
// NOTE: mockito 1.x does not support response delays (with_delay removed).
// This test is #[ignore] by default. Run with:
//   cargo test --test integration_tests -- --ignored
// to verify timeout behavior against a real slow endpoint.
// The From<reqwest::Error> mapping (TimeoutError) is covered by unit-level
// verification of reqwest::Error::is_timeout() in production error paths.

#[test]
#[ignore = "mockito 1.x lacks with_delay; requires real slow endpoint"]
fn test_timeout_returns_timeout_error() {
    // To test manually: point downloader at a slow endpoint (e.g. httpstat.us/200?sleep=5000)
    // with a short timeout and verify TimeoutError is returned.
    let downloader = ReqwestDownloader::with_timeout(Duration::from_secs(1));
    let job_id = JobId::new();
    let result = downloader.download("http://10.255.255.1/slow.pdf", &job_id);
    // 10.255.255.1 is a non-routable address — connection will timeout
    assert!(
        matches!(result, Err(InfrastructureError::TimeoutError(_))),
        "should return TimeoutError, got: {:?}",
        result
    );
}

// ===== Test 8: Validation errors do NOT trip circuit breaker =====
// F6: Validation errors are client-side data issues, not S3 outages.
// 5 consecutive invalid downloads should NOT open the circuit.

#[test]
fn test_validation_errors_do_not_open_circuit() {
    let mut server = mockito::Server::new();
    let _m = server
        .mock("GET", mockito::Matcher::Any)
        .with_status(200)
        .with_body(non_pdf_bytes())
        .create();

    let downloader = ReqwestDownloader::new();

    // 5 consecutive validation errors
    for i in 0..5 {
        let job_id = JobId::new();
        let url = format!("{}/invalid_{}.pdf", server.url(), i);
        let result = downloader.download(&url, &job_id);
        assert!(
            matches!(result, Err(InfrastructureError::ValidationError(_))),
            "call {} should return ValidationError, got: {:?}",
            i,
            result
        );
    }

    // 6th call with VALID content should succeed (circuit NOT open)
    let job_id = JobId::new();
    cleanup_job_files(&job_id);
    let url = format!("{}/valid.pdf", server.url());

    // Override the mock to return valid PDF
    let _m2 = server
        .mock("GET", "/valid.pdf")
        .with_status(200)
        .with_body(fake_pdf_bytes())
        .create();

    let result = downloader.download(&url, &job_id);
    assert!(
        result.is_ok(),
        "circuit should NOT be open after validation errors, got: {:?}",
        result
    );
}
