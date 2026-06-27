# Document Downloader Module

## Purpose

Downloads PDF documents from S3 URLs (or any HTTP URL) into the local temp directory
(`~/.sapo-printer/temp/`) for bulk printing. This module is the foundation of the
document processing pipeline in Epic 3.

## Architecture

### Trait-Based Design

```
DocumentDownloader (trait)
    └── ReqwestDownloader (concrete implementation)
            └── CircuitBreaker (resilience pattern)
```

The `DocumentDownloader` trait defines the service contract, enabling:
- Testability via mocking in unit tests
- Future replacement of the HTTP client without changing callers
- `Arc<dyn DocumentDownloader>` injection into use cases

### Circuit Breaker Rationale

When S3 is unavailable, naive retry logic creates **retry storms** — wasted CPU and
network resources hammering a failing service. The circuit breaker prevents this:

- **Closed** (normal): All requests pass through, failures counted
- **Open** (failing): After 5 consecutive failures, reject all requests immediately
- **HalfOpen** (testing): After 60s cooldown, allow one test request
  - Success → Closed (service recovered)
  - Failure → Open (back to cooldown)

### Atomic Download Pattern

Files are downloaded to `.tmp`, validated, then atomically renamed to `.pdf`:

```
~/.sapo-printer/temp/{job_id}.tmp   ← download + validate
                                    ↓ (only after validation passes)
~/.sapo-printer/temp/{job_id}.pdf   ← ready for printing
```

On any error, the `.tmp` file is deleted to prevent partial file leaks.

## Usage

```rust
use crate::infrastructure::integrations::network::{DocumentDownloader, ReqwestDownloader};
use crate::domain::models::JobId;

let downloader = ReqwestDownloader::new();
let job_id = JobId::new();
let path = downloader.download("https://s3.example.com/doc.pdf", &job_id)?;
// path → ~/.sapo-printer/temp/{job_id}.pdf
```

## Configuration

| Parameter          | Value   | Source              |
|--------------------|---------|---------------------|
| HTTP timeout       | 30s     | reqwest client      |
| Failure threshold  | 5       | CircuitBreaker      |
| Cooldown timeout   | 60s     | CircuitBreaker      |

## Testing

### Unit Tests

Inline `#[cfg(test)]` modules cover:
- **CircuitBreaker**: All FSM state transitions, threshold behavior
- **ReqwestDownloader**: PDF header validation, temp file cleanup, trait object safety

### Integration Tests

`tests/integration/downloader_integration_test.rs` uses `mockito` for:
- Successful PDF download → `.pdf` file created
- Invalid content → `ValidationError` + `.tmp` cleanup
- HTTP errors → `NetworkError`
- Circuit breaker → opens after 5 failures, returns `CircuitOpenError`
- End-to-end large file download + validation

Run: `cargo test` (includes both unit and integration tests)

## Error Handling

| Error                | When                          | Retryable?  |
|----------------------|-------------------------------|-------------|
| `NetworkError`       | HTTP failure, DNS, TLS        | Yes         |
| `TimeoutError`       | Request > 30s                 | Yes         |
| `ValidationError`    | Content not a valid PDF       | No          |
| `CircuitOpenError`   | Circuit breaker is open       | No (wait)   |

Retry logic is deferred to the queue worker level (Story 3.6), not implemented here.

## Future Enhancements

- Retry logic at queue worker level (Story 3.6)
- RAII `TempPdfFile` lifecycle management (Story 3.5)
- Download progress callbacks (post-MVP)
- Resume partial downloads via HTTP Range requests (post-MVP)
- S3 presigned URL refresh on expiry (post-MVP)
