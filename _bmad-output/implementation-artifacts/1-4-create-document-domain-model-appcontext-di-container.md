---
baseline_commit: "71cac14051cad9fbed4562ef13b42f69bd7bd55a"
---

# Story 1.4: Create Document Domain Model & AppContext DI Container

Status: done

## Story

As a **developer**,
I want **to implement the Document domain model and create the AppContext dependency injection container**,
So that **all domain models are complete and the application has a centralized DI pattern ready for use cases**.

## Acceptance Criteria

**Given** PrintJob and Printer domain models exist (Stories 1.2 + 1.3 done)
**When** I implement Document domain and AppContext

**AC-1: Document Value Objects**
- `src-tauri/src/domain/document/value_objects.rs` containing:
  - `DocumentId` — UUID v4 wrapper (same pattern as `JobId`, `PrinterId`)
  - `DocumentType` enum with `Pdf` variant
  - `DocumentLocation` — newtype wrapping `String` (validated URL)

**AC-2: Document Aggregate**
- `src-tauri/src/domain/document/aggregate.rs` containing `Document` aggregate with:
  - Fields: `id: DocumentId`, `doc_type: DocumentType`, `location: DocumentLocation`
  - `Document::new(location: DocumentLocation, doc_type: DocumentType) -> Result<Self, DocumentDomainError>`
  - Validation rule: `location` must be non-empty and start with `http://` or `https://`
  - All structs implement `Clone`, `Debug`, `Serialize`, `Deserialize`

**AC-3: Document Errors**
- `src-tauri/src/domain/document/errors.rs` containing `DocumentDomainError`:
  - `InvalidUrl { url: String }` — fired when URL fails validation

**AC-4: EventBus Trait**
- `src-tauri/src/shared/event_bus.rs` containing `EventBus` trait:
  - `publish(&self, event_type: &str, payload: &str) -> Result<(), EventBusError>` (simple string-based interface for now)
  - `EventBusError` enum with at least `PublishFailed { reason: String }` variant
  - Trait must be `Send + Sync`

**AC-5: AppContext DI Container**
- `src-tauri/src/shared/app_context.rs` containing `AppContext` struct:
  - Fields using `Arc<dyn Trait>`:
    - `pub job_repo: Arc<dyn PrintJobRepository>`
    - `pub printer_repo: Arc<dyn PrinterRepository>`
    - `pub event_bus: Arc<dyn EventBus>`
  - `AppContext::new(db_path: &str) -> Self` — constructor accepting database path
  - Platform-specific printer engine marker via conditional compilation:
    - `pub fn platform_engine_name() -> &'static str` returning `"windows"` on Windows, `"cups"` on non-Windows

**AC-6: Module Wiring**
- `src-tauri/src/domain/document/mod.rs` declares submodules and re-exports key types
- `src-tauri/src/shared/mod.rs` re-exports `event_bus` and `app_context` modules
- `cargo build` succeeds with zero errors

**AC-7: Unit Tests**
- Document domain inline tests (`#[cfg(test)]`) must cover:
  - `DocumentId::new()` generates unique IDs
  - `Document::new()` succeeds with valid HTTPS URL
  - `Document::new()` fails with empty string → `InvalidUrl`
  - `Document::new()` fails with non-HTTP URL (e.g., `"ftp://..."`) → `InvalidUrl`
- AppContext/EventBus tests must cover:
  - `AppContext::platform_engine_name()` returns non-empty string
  - `EventBusError::PublishFailed` Display contains the reason string

**AC-8: All Tests Pass**
- `cargo test` — all tests pass (no regressions to Stories 1.1–1.3)

**AC-9: Domain Independence**
- `domain/document/` has ZERO imports from `infrastructure/`, `application/`, or `interface/`
- Only `serde` and `uuid` crates in domain layer

## Tasks / Subtasks

- [x] **Task 1: Create Document Errors** (AC: #3, #9)
  - [x] Create `src-tauri/src/domain/document/errors.rs`
  - [x] Define `DocumentDomainError` enum with `InvalidUrl { url: String }`
  - [x] Implement `Display` and `std::error::Error`
  - [x] Add unit test: display message contains URL value

- [x] **Task 2: Create Document Value Objects** (AC: #1, #9)
  - [x] Create `src-tauri/src/domain/document/value_objects.rs`
  - [x] Implement `DocumentId` as UUID v4 wrapper: `new()`, `Display` impl
  - [x] Implement `DocumentType` enum: `Pdf` variant with derive macros
  - [x] Implement `DocumentLocation` as newtype `String`: `new(s: String) -> Self`, `as_str() -> &str`
  - [x] Add unit test: `DocumentId::new()` uniqueness

- [x] **Task 3: Create Document Aggregate** (AC: #2, #9)
  - [x] Create `src-tauri/src/domain/document/aggregate.rs`
  - [x] Implement `Document` struct with fields `id`, `doc_type`, `location`
  - [x] Implement `Document::new()` with URL validation (non-empty, starts with `http://` or `https://`)
  - [x] Add unit tests: valid URL succeeds, empty URL fails, `ftp://` URL fails

- [x] **Task 4: Wire Document Module** (AC: #6, #9)
  - [x] Update `src-tauri/src/domain/document/mod.rs` — replace stub comment with submodule declarations and re-exports
  - [x] Re-export: `Document`, `DocumentId`, `DocumentType`, `DocumentLocation`, `DocumentDomainError`

- [x] **Task 5: Define EventBus Trait** (AC: #4)
  - [x] Create `src-tauri/src/shared/event_bus.rs`
  - [x] Define `EventBusError` enum with `PublishFailed { reason: String }`; implement `Display` + `std::error::Error`
  - [x] Define `EventBus` trait: `publish(&self, event_type: &str, payload: &str) -> Result<(), EventBusError>` with `Send + Sync`
  - [x] Add unit test: `EventBusError::PublishFailed` display contains reason

- [x] **Task 6: Create AppContext** (AC: #5)
  - [x] Create `src-tauri/src/shared/app_context.rs`
  - [x] Add imports for `PrintJobRepository`, `PrinterRepository`, `EventBus`
  - [x] Define `AppContext` struct with three `Arc<dyn ...>` fields
  - [x] Implement `AppContext::new(db_path: &str) -> Self` — use `todo!()` placeholder for repo/bus construction (real impls come in Epic 2)
  - [x] Implement `platform_engine_name()` with `#[cfg(target_os = "windows")]` / `#[cfg(not(target_os = "windows"))]`
  - [x] Add unit test: `platform_engine_name()` returns non-empty string

- [x] **Task 7: Update Shared Module & Verify** (AC: #6, #8)
  - [x] Update `src-tauri/src/shared/mod.rs` to declare `pub mod event_bus;` and `pub mod app_context;`
  - [x] Run `cargo build` — must succeed with zero errors
  - [x] Run `cargo test` — all tests must pass (zero regressions)
  - [x] Run `cargo clippy` — fix any warnings

## Dev Notes

### 🎯 Story Purpose

This is the final story in Epic 1 (Foundation). It completes the domain layer (Document aggregate) and establishes the DI container (AppContext) that all Epic 2+ use cases will depend on.

**Key constraint:** `AppContext::new()` uses `todo!()` placeholders because `SqlitePrintJobRepository`, `SqlitePrinterRepository`, and `InMemoryEventBus` don't exist yet — they are Epic 2 stories. The goal here is the **interface** (struct shape, field types, constructor signature), not working implementations.

### 🏗️ Exact File Structure

```
NEW files:
  src-tauri/src/domain/document/errors.rs
  src-tauri/src/domain/document/value_objects.rs
  src-tauri/src/domain/document/aggregate.rs
  src-tauri/src/shared/event_bus.rs
  src-tauri/src/shared/app_context.rs

MODIFIED files:
  src-tauri/src/domain/document/mod.rs   (replace stub comment)
  src-tauri/src/shared/mod.rs            (add 2 pub mod declarations)
```

**DO NOT touch:** `domain/print_job/`, `domain/printer/` — complete and reviewed.

### 📦 Type Specifications

```rust
// domain/document/value_objects.rs
use uuid::Uuid;
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentId(Uuid);
impl DocumentId {
    pub fn new() -> Self { Self(Uuid::new_v4()) }
}
impl Default for DocumentId { fn default() -> Self { Self::new() } }
impl std::fmt::Display for DocumentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.0) }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentType { Pdf }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentLocation(String);
impl DocumentLocation {
    pub fn new(s: String) -> Self { Self(s) }
    pub fn as_str(&self) -> &str { &self.0 }
}
```

```rust
// domain/document/aggregate.rs
use serde::{Serialize, Deserialize};
use super::errors::DocumentDomainError;
use super::value_objects::{DocumentId, DocumentLocation, DocumentType};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    id: DocumentId,
    doc_type: DocumentType,
    location: DocumentLocation,
}

impl Document {
    pub fn new(location: DocumentLocation, doc_type: DocumentType) -> Result<Self, DocumentDomainError> {
        let url = location.as_str();
        if url.is_empty() || (!url.starts_with("http://") && !url.starts_with("https://")) {
            return Err(DocumentDomainError::InvalidUrl { url: url.to_string() });
        }
        Ok(Self { id: DocumentId::new(), doc_type, location })
    }
    pub fn id(&self) -> &DocumentId { &self.id }
    pub fn doc_type(&self) -> &DocumentType { &self.doc_type }
    pub fn location(&self) -> &DocumentLocation { &self.location }
}
```

```rust
// shared/event_bus.rs
use std::fmt;

#[derive(Debug)]
pub enum EventBusError {
    PublishFailed { reason: String },
}
impl fmt::Display for EventBusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventBusError::PublishFailed { reason } => write!(f, "Event publish failed: {}", reason),
        }
    }
}
impl std::error::Error for EventBusError {}

pub trait EventBus: Send + Sync {
    fn publish(&self, event_type: &str, payload: &str) -> Result<(), EventBusError>;
}
```

```rust
// shared/app_context.rs
use std::sync::Arc;
use crate::domain::print_job::PrintJobRepository;
use crate::domain::printer::PrinterRepository;
use crate::shared::event_bus::EventBus;

pub struct AppContext {
    pub job_repo: Arc<dyn PrintJobRepository>,
    pub printer_repo: Arc<dyn PrinterRepository>,
    pub event_bus: Arc<dyn EventBus>,
}

impl AppContext {
    pub fn new(_db_path: &str) -> Self {
        // Infrastructure implementations come in Epic 2.
        // Using todo!() placeholders — real repos wired in Story 2.4.
        todo!("AppContext::new — infrastructure not yet implemented (Epic 2)")
    }

    pub fn platform_engine_name() -> &'static str {
        #[cfg(target_os = "windows")]
        { "windows" }
        #[cfg(not(target_os = "windows"))]
        { "cups" }
    }
}
```

### ⚠️ AppContext::new() todo!() Pattern

`AppContext::new()` MUST use `todo!()` — this is intentional. Do NOT attempt to construct real implementations. The function signature is what matters for this story. `platform_engine_name()` is a separate `fn` that IS fully implemented.

The unit test for AppContext only tests `platform_engine_name()` — it does NOT call `AppContext::new()`.

### 🔗 Import Paths — Exact

`app_context.rs` imports:
```rust
use crate::domain::print_job::PrintJobRepository;
use crate::domain::printer::PrinterRepository;
use crate::shared::event_bus::EventBus;
```

These already exist:
- `PrintJobRepository` — `src-tauri/src/domain/print_job/repository.rs`
- `PrinterRepository` — `src-tauri/src/domain/printer/repository.rs`

`EventBus` is defined in this same story (Task 5 before Task 6).

### 🔁 Pattern Alignment with Story 1.2 / 1.3

Follow the EXACT same patterns from `print_job/` and `printer/`:
- `DocumentId`: same as `JobId` (uuid, Display, Default, FromStr optional)
- `DocumentDomainError`: same as `PrinterDomainError` (Display + std::error::Error, no Clone needed unless tests require)
- `now_unix()`: NOT needed here — Document has no events in this story
- Events buffer: NOT needed here — Document aggregate has no domain events in Story 1.4

### 🧪 Required Tests

| Test | File | Assertion |
|------|------|-----------|
| `test_document_id_unique` | value_objects.rs | Two `DocumentId::new()` calls differ |
| `test_document_new_valid_https` | aggregate.rs | `https://` URL → `Ok(Document)` |
| `test_document_new_valid_http` | aggregate.rs | `http://` URL → `Ok(Document)` |
| `test_document_new_empty_url` | aggregate.rs | `""` → `Err(InvalidUrl)` |
| `test_document_new_invalid_scheme` | aggregate.rs | `"ftp://example.com"` → `Err(InvalidUrl)` |
| `test_invalid_url_display` | errors.rs | Display contains the URL string |
| `test_event_bus_error_display` | event_bus.rs | `PublishFailed` display contains reason |
| `test_platform_engine_name_nonempty` | app_context.rs | `platform_engine_name()` is non-empty |

### 🚫 Anti-Patterns (DO NOT)

1. ❌ DO NOT add `events` buffer to Document — no domain events in this story
2. ❌ DO NOT implement `AppContext::new()` — use `todo!()`, real wiring is Epic 2
3. ❌ DO NOT create `InMemoryEventBus` implementation — only define the trait
4. ❌ DO NOT import from `infrastructure/` in domain layer
5. ❌ DO NOT use `async` — all synchronous
6. ❌ DO NOT add new Cargo.toml dependencies — all needed crates already present
7. ❌ DO NOT modify `domain/print_job/` or `domain/printer/`

### 📚 References

- AR-10 (DI Pattern): `_bmad-output/planning-artifacts/architecture.md` § "Dependency Injection Pattern"
- AR-2 (Event-Driven): `_bmad-output/planning-artifacts/architecture.md` § "Decision 2"
- Story 1.2 patterns: `_bmad-output/implementation-artifacts/1-2-create-printjob-domain-model.md`
- Story 1.3 patterns: `_bmad-output/implementation-artifacts/1-3-create-printer-domain-model.md`
- Epic 1, Story 1.4 AC: `_bmad-output/planning-artifacts/epics.md` § "Story 1.4"

## Dev Agent Record

### Agent Model Used

Kiro (Claude) — Amelia persona

### Debug Log References

Không có vấn đề. Build sạch, clippy clean.

### Completion Notes List

✅ **Task 1** — `domain/document/errors.rs`: `DocumentDomainError::InvalidUrl`, Display, std::error::Error. 1 test.

✅ **Task 2** — `domain/document/value_objects.rs`: `DocumentId` (UUID v4), `DocumentType::Pdf`, `DocumentLocation` newtype. 1 test.

✅ **Task 3** — `domain/document/aggregate.rs`: `Document::new()` với URL validation (empty + non-http scheme → `InvalidUrl`). 4 tests.

✅ **Task 4** — `domain/document/mod.rs`: submodule declarations + re-exports.

✅ **Task 5** — `shared/event_bus.rs`: `EventBusError::PublishFailed`, `EventBus` trait (Send + Sync). 1 test.

✅ **Task 6** — `shared/app_context.rs`: `AppContext` struct với 3 `Arc<dyn ...>` fields, `new()` dùng `todo!()`, `platform_engine_name()` với conditional compilation. 1 test.

✅ **Task 7** — `shared/mod.rs` cập nhật. `cargo build` ✅ · `cargo test` 91/91 passed ✅ · `cargo clippy` 0 warnings ✅

Domain layer hoàn toàn độc lập: `domain/document/` không có import nào từ infrastructure/application/interface.

### File List

**New files:**
- `src-tauri/src/domain/document/errors.rs`
- `src-tauri/src/domain/document/value_objects.rs`
- `src-tauri/src/domain/document/aggregate.rs`
- `src-tauri/src/shared/event_bus.rs`
- `src-tauri/src/shared/app_context.rs`

**Modified files:**
- `src-tauri/src/domain/document/mod.rs`
- `src-tauri/src/shared/mod.rs`

## Change Log

**2026-06-22: Story 1.4 Implementation Complete**
- Tạo Document domain model: `DocumentId`, `DocumentType`, `DocumentLocation`, `Document` aggregate với URL validation
- Tạo `EventBus` trait trong shared layer
- Tạo `AppContext` DI container với platform-specific engine marker
- 91 tests passing (8 mới + 83 từ stories 1.1–1.3), 0 regressions
- `cargo build`, `cargo test`, `cargo clippy` đều clean


## Review Findings

- [x] [Review][Patch] `DocumentDomainError` missing `Clone` derive — fixed: added `Clone` to derive [`src-tauri/src/domain/document/errors.rs:3`]
- [x] [Review][Defer] Scheme-only URL (`https://`) passes guard — no host presence check [`src-tauri/src/domain/document/aggregate.rs:18`] — deferred, pre-existing
- [x] [Review][Defer] Uppercase scheme `HTTP://`/`HTTPS://` rejected incorrectly — scheme check is case-sensitive [`src-tauri/src/domain/document/aggregate.rs:18`] — deferred, pre-existing
- [x] [Review][Defer] URL with leading whitespace rejected — no trim before validation [`src-tauri/src/domain/document/aggregate.rs:18`] — deferred, pre-existing
- [x] [Review][Defer] `DocumentLocation::new` accepts any string without URL validation — intentional per spec design [`src-tauri/src/domain/document/value_objects.rs:35`] — deferred, pre-existing
- [x] [Review][Defer] HTTP URLs accepted — MITM/plaintext risk — ngoài scope story 1.4, security hardening Epic 2+ [`src-tauri/src/domain/document/aggregate.rs:23`] — deferred, pre-existing
- [x] [Review][Defer] `AppContext::new` panics unconditionally — intentional `todo!()` per spec dev notes [`src-tauri/src/shared/app_context.rs:14`] — deferred, pre-existing
- [x] [Review][Defer] `AppContext` fields all `pub` — encapsulation post-Epic-2 refactor [`src-tauri/src/shared/app_context.rs:13`] — deferred, pre-existing
- [x] [Review][Defer] `EventBus::publish` untyped `&str` — intentional per spec "simple string-based interface for now" [`src-tauri/src/shared/event_bus.rs:24`] — deferred, pre-existing
- [x] [Review][Defer] `DocumentId` no `From<Uuid>`/accessor for persistence reconstruction — Epic 2+ concern [`src-tauri/src/domain/document/value_objects.rs:4`] — deferred, pre-existing
- [x] [Review][Defer] `DocumentType` missing `#[non_exhaustive]` — valid concern, ngoài scope story 1.4 [`src-tauri/src/domain/document/value_objects.rs:28`] — deferred, pre-existing
- [x] [Review][Defer] `platform_engine_name` returns `"cups"` on non-desktop targets — project scope là Tauri desktop only [`src-tauri/src/shared/app_context.rs:29`] — deferred, pre-existing
