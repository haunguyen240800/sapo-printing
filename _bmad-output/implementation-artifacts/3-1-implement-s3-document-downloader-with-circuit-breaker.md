---
baseline_commit: 6cf96beab8230f12f86402d8d3ca5600d96b1c00
---

# Story 3.1: Implement S3 Document Downloader with Circuit Breaker

Status: done

## Story

As a **developer**,
I want **to implement a resilient document downloader with circuit breaker for S3 URLs**,
So that **the app can download PDFs reliably and fail gracefully when S3 is unavailable**.

## Context

Story này là **story đầu tiên trong Epic 3 (Bulk Print Job Management with Document Processing Pipeline)** và xây dựng **critical first piece** của document processing pipeline: downloading PDFs từ S3 với resilience patterns.

**Business Value:**
- Enable bulk print feature bằng cách download PDFs từ S3 URLs
- Graceful degradation khi S3 unavailable (circuit breaker prevents retry storms)
- Foundation cho toàn bộ document processing pipeline (Stories 3.2-3.9 depend on this)

**Architecture Context:**
- Infrastructure layer implementation — implements service contract (DocumentDownloader trait)
- Circuit Breaker pattern prevents wasted CPU/network khi S3 down (Risk 4 mitigation from architecture)
- Atomic download pattern prevents partial file leaks (Risk 5 mitigation)
- RAII pattern integration với TempPdfFile (Story 3.5) cho automatic cleanup

**Epic Dependencies:**
- **Depends on Epic 1:** Project structure, AppContext DI container, JobId value object
- **Depends on Epic 2:** SQLite database setup, temp directory (`~/.sapo-printer/temp/`)
- **Blocks Stories 3.2-3.9:** Rendering, printing, và temp file management đều cần downloaded PDFs

## Acceptance Criteria

### AC-1: DocumentDownloader Trait Definition

**Given** the infrastructure layer structure exists  
**When** I define the service contract  
**Then** the following must be created in `src-tauri/src/infrastructure/downloader/document_downloader.rs`:
- DocumentDownloader trait with method:
  - `fn download(&self, url: Url, job_id: JobId) -> Result<PathBuf, InfrastructureError>`
- Trait bounds: `Send + Sync` for use with `Arc<dyn DocumentDownloader>`
- Method contract documentation:
  - Downloads file from `url` to temp directory
  - Returns PathBuf to final `.pdf` file (not `.tmp`)
  - Errors: NetworkError, ValidationError, TimeoutError
- Module-level documentation explaining:
  - Purpose: Download documents from S3 URLs for printing
  - Implementation note: Concrete implementations should validate file type
  - Integration: Used by QueueWorker in Story 3.5

**And** `src-tauri/src/infrastructure/downloader/mod.rs` exports:
```rust
pub mod document_downloader;
pub mod reqwest_downloader;
pub mod circuit_breaker;

pub use document_downloader::DocumentDownloader;
pub use reqwest_downloader::ReqwestDownloader;
pub use circuit_breaker::CircuitBreaker;
```

### AC-2: ReqwestDownloader Implementation

**Given** the DocumentDownloader trait exists  
**When** I implement the reqwest-based downloader  
**Then** `src-tauri/src/infrastructure/downloader/reqwest_downloader.rs` must be created:
- Struct: `pub struct ReqwestDownloader { client: reqwest::blocking::Client, circuit_breaker: Mutex<CircuitBreaker> }`
- Constructor: `pub fn new() -> Self` initializes reqwest client với 30s timeout
- Implement DocumentDownloader trait:
  - `download(url: Url, job_id: JobId) -> Result<PathBuf, InfrastructureError>`
- **Atomic download pattern:**
  1. Check circuit breaker state (return CircuitOpenError if Open)
  2. Construct temp path: `~/.sapo-printer/temp/{job_id}.tmp`
  3. Construct final path: `~/.sapo-printer/temp/{job_id}.pdf`
  4. Download to `.tmp` file using reqwest (wrap in circuit breaker call)
  5. Validate PDF header: read first 4 bytes, verify equals `%PDF`
  6. Atomic rename: `fs::rename(temp_path, final_path)?`
  7. Return final path
  8. On error: delete `.tmp` file if exists, update circuit breaker
- **Timeout:** 30 seconds per FR-3.1 (configured in reqwest client)
- **Error handling:**
  - Network errors → InfrastructureError::NetworkError, trigger circuit breaker
  - Invalid PDF → InfrastructureError::ValidationError, cleanup `.tmp`, trigger circuit breaker
  - Timeout → InfrastructureError::TimeoutError, cleanup `.tmp`, trigger circuit breaker
  - Circuit open → InfrastructureError::CircuitOpenError (immediate return, no download attempt)

**And** helper function `validate_pdf_header(path: &Path) -> Result<(), InfrastructureError>`:
- Read first 5 bytes from file
- Check if starts with `%PDF-` (PDF signature)
- Return ValidationError if invalid

**And** inline unit tests (`#[cfg(test)]`) verify:
- Valid PDF download succeeds and returns `.pdf` path (mock HTTP server)
- Invalid file (non-PDF) fails validation
- `.tmp` file cleaned up on validation failure
- Timeout after 30s triggers TimeoutError
- Circuit breaker integration (failures increment failure count)

### AC-3: Circuit Breaker Implementation

**Given** the need for resilience against S3 outages  
**When** I implement the circuit breaker  
**Then** `src-tauri/src/infrastructure/downloader/circuit_breaker.rs` must be created:
- **States enum:**
  ```rust
  pub enum CircuitState {
      Closed,   // Normal operation
      Open,     // Failing, reject all calls
      HalfOpen, // Testing if recovered
  }
  ```
- **CircuitBreaker struct:**
  ```rust
  pub struct CircuitBreaker {
      state: CircuitState,
      failure_count: u32,
      failure_threshold: u32,  // = 5
      success_count: u32,      // For HalfOpen state
      open_until: Option<Instant>,
      timeout: Duration,       // = 60s
  }
  ```
- **Methods:**
  - `pub fn new() -> Self` — Initialize in Closed state, threshold=5, timeout=60s
  - `pub fn call<F, T>(&mut self, f: F) -> Result<T, InfrastructureError>` — Execute operation through circuit breaker
  - `fn on_success(&mut self)` — Reset failure count, transition Open→HalfOpen→Closed
  - `fn on_failure(&mut self)` — Increment failure count, transition Closed→Open if threshold exceeded
  - `fn should_attempt(&self) -> bool` — Check if call should be attempted based on state
- **State transitions:**
  - **Closed → Open:** After 5 consecutive failures
  - **Open → HalfOpen:** After 60s timeout expires
  - **HalfOpen → Closed:** After 1 successful call in HalfOpen
  - **HalfOpen → Open:** After any failure in HalfOpen
- **Behavior:**
  - **Closed:** All calls pass through, count failures
  - **Open:** Immediately return CircuitOpenError (no network call), wait for timeout
  - **HalfOpen:** Allow single test call, success→Closed, failure→Open

**And** inline unit tests verify:
- Circuit breaker opens after 5 consecutive failures
- Circuit breaker returns CircuitOpenError immediately when Open
- Circuit breaker transitions Open → HalfOpen after 60s
- Circuit breaker transitions HalfOpen → Closed on success
- Circuit breaker transitions HalfOpen → Open on failure
- Successful calls reset failure count in Closed state

### AC-4: Dependencies Configuration

**Given** the implementation uses external HTTP client  
**When** I configure dependencies  
**Then** `src-tauri/Cargo.toml` must include:
```toml
[dependencies]
reqwest = { version = "0.11", features = ["blocking"] }
url = "2.5"
```

**And** existing dependencies are used:
- `uuid` — for JobId (from Story 1.2)
- `std::fs` — for file operations
- `std::time` — for Instant, Duration

### AC-5: InfrastructureError Extension

**Given** the downloader needs specific error types  
**When** I extend the error enum  
**Then** `src-tauri/src/shared/errors/infrastructure_error.rs` must be updated:
- Add error variants:
  ```rust
  pub enum InfrastructureError {
      // ... existing variants
      NetworkError(String),
      ValidationError(String),
      TimeoutError(String),
      CircuitOpenError,
  }
  ```
- Implement `Display` for new variants với clear messages
- Implement `From<reqwest::Error>` for `InfrastructureError` → map to NetworkError hoặc TimeoutError

### AC-6: AppContext Integration (Placeholder)

**Given** the downloader implementation exists  
**When** I prepare for AppContext integration  
**Then** add a TODO comment in `src-tauri/src/shared/app_context.rs`:
```rust
// TODO (Story 3.1): Add DocumentDownloader to AppContext
// pub downloader: Arc<dyn DocumentDownloader>,
// Initialize in AppContext::new():
// downloader: Arc::new(ReqwestDownloader::new()),
```

**Note:** Full integration will happen when Use Cases are created (Story 3.8 onwards)

### AC-7: Integration Test with Mock S3 Endpoint

**Given** all components implemented  
**When** I create integration test  
**Then** `src-tauri/tests/integration/downloader_integration_test.rs` must be created:
- Setup mock HTTP server using `mockito` crate (dev-dependency)
- Test scenarios:
  1. **Successful download:** Mock returns valid PDF bytes, verify `.pdf` file created
  2. **Invalid content:** Mock returns non-PDF bytes, verify ValidationError và `.tmp` cleanup
  3. **Timeout:** Mock delays > 30s, verify TimeoutError
  4. **Circuit breaker:** Trigger 5 failures, verify 6th call returns CircuitOpenError immediately
  5. **Circuit recovery:** Wait 60s after circuit opens, verify HalfOpen allows retry
- All tests must pass with `cargo test --test downloader_integration_test`

**And** add dev-dependency to Cargo.toml:
```toml
[dev-dependencies]
mockito = "1.0"
```

### AC-8: Documentation & Examples

**Given** the implementation is complete  
**When** I document the module  
**Then** `src-tauri/src/infrastructure/downloader/README.md` must be created với:
- Module purpose: Download documents from S3 URLs for bulk printing
- Architecture: Trait-based design for testability
- Circuit breaker rationale: Prevent retry storms khi S3 down
- Usage example:
  ```rust
  let downloader = ReqwestDownloader::new();
  let path = downloader.download(url, job_id)?;
  // path points to: ~/.sapo-printer/temp/{job_id}.pdf
  ```
- Testing notes: Use mockito for integration tests
- Future enhancements: Retry logic at queue worker level (Story 3.6)

## Tasks / Subtasks

### Task 1: Define DocumentDownloader Trait (AC-1)
- [ ] Create `src-tauri/src/infrastructure/downloader/` directory
- [ ] Create `document_downloader.rs` với trait definition
- [ ] Add trait method: `download(url, job_id) -> Result<PathBuf>`
- [ ] Add trait bounds: `Send + Sync`
- [ ] Document trait contract (preconditions, postconditions, errors)
- [ ] Create `mod.rs` và export trait

### Task 2: Implement ReqwestDownloader (AC-2)
- [ ] Create `reqwest_downloader.rs`
- [ ] Define struct với reqwest client và circuit breaker
- [ ] Implement `new()` constructor với 30s timeout configuration
- [ ] Implement `download()` method:
  - [ ] Check circuit breaker state
  - [ ] Generate temp path (`{job_id}.tmp`)
  - [ ] Download to temp file via reqwest
  - [ ] Validate PDF header (helper function)
  - [ ] Atomic rename to `.pdf`
  - [ ] Error handling và cleanup
- [ ] Write inline unit tests với mock HTTP responses
- [ ] Verify all edge cases (timeout, invalid PDF, cleanup)

### Task 3: Implement Circuit Breaker (AC-3)
- [ ] Create `circuit_breaker.rs`
- [ ] Define `CircuitState` enum (Closed, Open, HalfOpen)
- [ ] Define `CircuitBreaker` struct với state fields
- [ ] Implement `new()` — default config (threshold=5, timeout=60s)
- [ ] Implement `call()` — wrap operation in circuit breaker
- [ ] Implement `on_success()` — reset failures, transition states
- [ ] Implement `on_failure()` — increment failures, check threshold
- [ ] Implement `should_attempt()` — check if Open với timeout expired
- [ ] Write inline unit tests cho all state transitions
- [ ] Verify threshold và timeout behavior

### Task 4: Configure Dependencies & Errors (AC-4, AC-5)
- [ ] Add `reqwest` và `url` to Cargo.toml dependencies
- [ ] Extend `InfrastructureError` enum với new variants
- [ ] Implement `Display` for new error variants
- [ ] Implement `From<reqwest::Error>` conversion
- [ ] Add `mockito` to dev-dependencies

### Task 5: Integration Test (AC-7)
- [ ] Create `tests/integration/` directory if not exists
- [ ] Create `downloader_integration_test.rs`
- [ ] Setup mockito mock server
- [ ] Test scenario 1: Successful PDF download
- [ ] Test scenario 2: Invalid content validation
- [ ] Test scenario 3: Timeout handling
- [ ] Test scenario 4: Circuit breaker opens after 5 failures
- [ ] Test scenario 5: Circuit recovery after timeout
- [ ] Verify all integration tests pass

### Task 6: Documentation & AppContext Placeholder (AC-6, AC-8)
- [ ] Add TODO comment in AppContext for future integration
- [ ] Create `README.md` trong downloader module
- [ ] Document architecture rationale
- [ ] Add usage examples
- [ ] Document testing strategy

### Task 7: Final Verification
- [ ] Run `cargo test` — all unit tests pass
- [ ] Run `cargo test --test downloader_integration_test` — integration tests pass
- [ ] Run `cargo clippy` — no warnings
- [ ] Run `cargo fmt` — code formatted
- [ ] Verify file structure matches AC-1
- [ ] Verify temp directory handling matches architecture Decision 12

## Dev Notes

### Architecture Alignment

**Clean Architecture + DDD (AR-1):**
- Story này thuộc **Infrastructure Layer** — implements technical concerns (HTTP download, circuit breaker)
- DocumentDownloader là **service contract (trait)** — không phải domain repository
- Domain layer không biết gì về HTTP, S3, hoặc reqwest — maintains independence

**Dependency Inversion Principle:**
- Trait defined trong infrastructure (acceptable for service contracts)
- AppContext sẽ inject `Arc<dyn DocumentDownloader>` vào Use Cases (Story 3.8+)
- Testable via trait mocking

**Circuit Breaker Pattern (AR-6 + Risk 4 Mitigation):**
- **Purpose:** Prevent retry storms khi S3 down (wasted CPU/network)
- **Pattern:** State machine với 3 states (Closed, Open, HalfOpen)
- **Threshold:** 5 consecutive failures → Open (tunable if needed)
- **Timeout:** 60s in Open before attempting HalfOpen test
- **Monitoring:** Log warnings when circuit opens (Story 4.3: structured logging)

**RAII Pattern for Resource Cleanup (AR-7):**
- Atomic download pattern: `.tmp` → `.pdf` only after validation
- On error: cleanup `.tmp` immediately
- Integration với TempPdfFile struct (Story 3.5) cho full lifecycle management
- Prevents partial file leaks (Risk 5 mitigation)

**Error Handling Strategy (AR-14):**
- Use InfrastructureError (3-tier error system)
- Distinguish error types: NetworkError, ValidationError, TimeoutError, CircuitOpenError
- Enable smart retry logic ở queue worker level (Story 3.6):
  - NetworkError → retryable
  - TimeoutError → retryable
  - ValidationError → non-retryable (bad URL/content)
  - CircuitOpenError → non-retryable (wait for recovery)

### File Structure

```
src-tauri/src/infrastructure/downloader/
├── mod.rs                          # Module exports
├── document_downloader.rs          # Trait definition (service contract)
├── reqwest_downloader.rs           # Implementation (reqwest-based)
│   └── #[cfg(test)] mod tests     # Inline unit tests
├── circuit_breaker.rs              # Circuit breaker pattern
│   └── #[cfg(test)] mod tests     # State transition tests
└── README.md                       # Module documentation

src-tauri/tests/integration/
└── downloader_integration_test.rs  # Integration tests với mockito
```

### Testing Strategy

**Unit Tests (Inline #[cfg(test)]):**
- ReqwestDownloader: Mock HTTP responses, verify error handling
- CircuitBreaker: State machine transitions, threshold behavior
- validate_pdf_header: Valid/invalid PDF signatures

**Integration Tests (tests/integration/):**
- Real reqwest client với mockito mock server
- End-to-end flow: HTTP request → download → validation → file creation
- Circuit breaker integration: Verify actual state changes affect download calls

**Coverage Targets:**
- All error paths (network fail, timeout, invalid PDF)
- Circuit breaker state machine (all transitions)
- Atomic rename pattern (no partial files)

### Implementation Guidance

**Reqwest Configuration:**
```rust
use reqwest::blocking::Client;
use std::time::Duration;

let client = Client::builder()
    .timeout(Duration::from_secs(30))
    .build()?;
```

**PDF Header Validation:**
```rust
fn validate_pdf_header(path: &Path) -> Result<(), InfrastructureError> {
    let mut file = File::open(path)?;
    let mut header = [0u8; 5];
    file.read_exact(&mut header)?;
    
    if &header != b"%PDF-" {
        return Err(InfrastructureError::ValidationError(
            "Invalid PDF header".into()
        ));
    }
    Ok(())
}
```

**Atomic Rename Pattern:**
```rust
let temp_path = format!("~/.sapo-printer/temp/{}.tmp", job_id);
let final_path = format!("~/.sapo-printer/temp/{}.pdf", job_id);

// Download to temp
client.get(url).send()?.copy_to(&mut File::create(&temp_path)?)?;

// Validate
validate_pdf_header(Path::new(&temp_path))?;

// Atomic rename (only after validation)
fs::rename(&temp_path, &final_path)?;
```

**Circuit Breaker Integration:**
```rust
impl ReqwestDownloader {
    fn download(&self, url: Url, job_id: JobId) -> Result<PathBuf> {
        let mut cb = self.circuit_breaker.lock().unwrap();
        
        cb.call(|| {
            // Actual download logic here
            self.download_internal(url, job_id)
        })
    }
}
```

### Key Constraints from Architecture

**Performance (NFR-1):**
- PDF download < 30s per document (enforced via reqwest timeout)
- Concurrent downloads: Max 10 parallel (enforced at queue worker level, not here)

**Reliability (NFR-2):**
- Timeout protection: 30s reqwest client configuration
- Graceful degradation: Circuit breaker prevents cascading failures

**From Decision 12 (Temp File Management):**
- Temp location: `~/.sapo-printer/temp/` (user home directory, not system temp)
- Atomic pattern: `.tmp` → `.pdf` only after validation
- Cleanup: Manual in this story (`.tmp` on error), automatic lifecycle in Story 3.5

**From Risk 4 (Circuit Breaker):**
- Threshold: 5 failures (conservative, can tune based on production data)
- Timeout: 60s (balance between fast recovery vs avoiding flapping)
- Monitoring: Log circuit state changes (use `tracing::warn!` when opening)

**From Risk 5 (Partial Downloads):**
- Atomic rename prevents partial files being used
- Cleanup `.tmp` on any error (network, timeout, validation)
- Startup cleanup in Story 3.5 will handle orphaned `.tmp` files

### Previous Story Learnings

**From Story 2.6 (Secret Management):**
- Platform abstraction via traits works well
- Inline unit tests (`#[cfg(test)]`) keep tests close to code
- Clear error handling với specific error types improves debugging
- Document platform-specific quirks trong code comments

**From Story 2.1 (Database Setup):**
- Unit conversion utilities established trong `shared/utils/` (though not used here)
- Temp directory established: `~/.sapo-printer/temp/`

**Pattern to Follow:**
- Trait definition first (service contract)
- Single concrete implementation (ReqwestDownloader)
- Inline unit tests covering all branches
- Integration test with real dependencies (mockito)
- Clear documentation trong module README

**Pattern to Avoid:**
- ❌ Don't implement retry logic here — belongs in queue worker (Story 3.6)
- ❌ Don't add progress tracking — belongs in Use Case layer (Story 3.8+)
- ❌ Don't integrate with AppContext yet — defer to Use Case stories
- ❌ Don't implement RAII TempPdfFile here — that's Story 3.5

### References

**Source: _bmad-output/planning-artifacts/epics.md**
- Story 3.1 definition (lines 729-765)
- Acceptance Criteria verbatim
- Epic 3 context (lines 715-728)

**Source: _bmad-output/planning-artifacts/architecture.md**
- Circuit Breaker Pattern (Risk 4, lines 616-652)
- Atomic Download Pattern (Risk 5, lines 656-676)
- Temp File Management (Decision 12, lines 256-276)
- Infrastructure Layer structure (lines 2499-2502)
- Service Boundaries (lines 2638-2640)
- Error Handling Strategy (lines 354-380)
- AppContext DI Pattern (lines 1383-1424)
- Testing Strategy (lines 442-474)

**Source: CLAUDE.md (Project Instructions)**
- Clean Architecture 4-layer structure
- Domain layer independence (no external dependencies)
- Infrastructure implements domain contracts
- Event-Driven Architecture (not used in this story)

### Known Limitations & Future Work

**Current Scope (Story 3.1):**
- ✅ Download single PDF from URL
- ✅ Validate PDF header
- ✅ Circuit breaker for resilience
- ✅ Atomic rename pattern

**Out of Scope (Deferred):**
- ❌ Retry logic → Story 3.6 (Auto-Retry Logic with Exponential Backoff)
- ❌ Concurrent download management → Story 3.5 (Queue Worker)
- ❌ Progress tracking → Story 3.8+ (Use Cases + UI)
- ❌ RAII TempPdfFile → Story 3.5 (RAII Temp File Management)
- ❌ Download batch optimization → Story 3.5 (Batch Processing)
- ❌ Metrics collection → Story 4.5 (Metrics Collection)

**Future Enhancements (Post-MVP):**
- Resume partial downloads (HTTP Range requests)
- Parallel chunk downloads for large files
- Download progress callbacks
- Content-Type validation beyond PDF header
- S3 presigned URL support (currently assumes public URLs)

## Dev Agent Record

### Completion Notes

Story 3.1 implements **resilient document downloading** với circuit breaker pattern. Key deliverables:

1. **DocumentDownloader trait** — service contract cho download operations
2. **ReqwestDownloader** — reqwest-based implementation với 30s timeout
3. **CircuitBreaker** — prevents S3 retry storms (5 failures → 60s cooldown)
4. **Atomic download pattern** — `.tmp` → `.pdf` only after validation
5. **PDF header validation** — ensures downloaded files are valid PDFs
6. **Comprehensive tests** — unit (circuit breaker FSM) + integration (mockito)

This story **unblocks** Stories 3.2-3.9 by providing the foundation for document processing pipeline.

### File List

**Created:**
- `src-tauri/src/infrastructure/downloader/mod.rs`
- `src-tauri/src/infrastructure/downloader/document_downloader.rs`
- `src-tauri/src/infrastructure/downloader/reqwest_downloader.rs`
- `src-tauri/src/infrastructure/downloader/circuit_breaker.rs`
- `src-tauri/src/infrastructure/downloader/README.md`
- `src-tauri/tests/integration/downloader_integration_test.rs`

**Modified:**
- `src-tauri/Cargo.toml` (add reqwest, url, mockito dependencies)
- `src-tauri/src/shared/errors/infrastructure_error.rs` (add NetworkError, ValidationError, TimeoutError, CircuitOpenError)
- `src-tauri/src/shared/app_context.rs` (add TODO comment for future integration)

### Agent Model Used

Context filled by bmad-create-story workflow
