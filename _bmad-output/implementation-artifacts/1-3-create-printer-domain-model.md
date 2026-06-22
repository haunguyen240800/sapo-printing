---
baseline_commit: ""
---

# Story 1.3: Create Printer Domain Model

Status: done

## Story

As a **developer**,
I want **to implement the complete Printer domain model (aggregate, events, value objects, repository trait)**,
So that **printer management business logic is defined independently and can be implemented across platforms**.

## Acceptance Criteria

**Given** the domain layer structure exists (created in Story 1.1)
**When** I implement the Printer domain model
**Then** the following must be created in `src-tauri/src/domain/printer/`:

**AC-1: Value Objects**
- `value_objects.rs` containing:
  - `PrinterId` — UUID v4 wrapper (stable identifier, distinct from printer name)
  - `PrinterName` — newtype wrapper around `String` (used by PrintJob to reference a printer)
  - `PrinterStatus` enum with 3 states: `Online`, `Offline`, `Error`
  - `PrinterType` enum (at minimum: `Local`, `Network`)

**AC-2: Domain Events**
- `events.rs` containing:
  - `PrinterConnected` — fired when printer comes online (status → Online)
  - `PrinterDisconnected` — fired when printer goes offline (status → Offline/Error)
  - Both events hold `printer_id: PrinterId` and `timestamp: u64` (Unix epoch seconds)

**AC-3: Aggregate Root**
- `aggregate.rs` containing `Printer` aggregate with:
  - Fields: `id: PrinterId`, `name: PrinterName`, `status: PrinterStatus`, `printer_type: PrinterType`, `events: Vec<Box<dyn PrinterEvent>>`
  - `Printer::new(name: PrinterName, printer_type: PrinterType) -> Self` — sets status to `Offline`, pushes no event
  - `connect(&mut self) -> Result<(), DomainError>` — transitions to `Online`, pushes `PrinterConnected`
  - `disconnect(&mut self) -> Result<(), DomainError>` — transitions to `Offline`, pushes `PrinterDisconnected`
  - `set_error(&mut self) -> Result<(), DomainError>` — transitions to `Error`, pushes `PrinterDisconnected`
  - `can_accept_job(&self) -> bool` — returns `true` only when status is `Online`
  - `drain_events() -> Vec<Box<dyn PrinterEvent>>` — returns and clears internal buffer
- Business rules:
  - Only `Online` printers can receive print jobs (`can_accept_job()`)
  - Printer status must be validated before job assignment

**AC-4: Repository Trait**
- `repository.rs` containing `PrinterRepository` trait:
  - `save(&self, printer: &Printer) -> Result<(), DomainError>`
  - `find_all(&self) -> Result<Vec<Printer>, DomainError>`
  - `find_by_name(&self, name: &PrinterName) -> Result<Option<Printer>, DomainError>`

**AC-5: Trait Derivations**
- All structs/enums must implement: `Clone`, `Debug`, `Serialize`, `Deserialize`
- `PrinterStatus` and `PrinterType` must implement `PartialEq`, `Eq`

**AC-6: Unit Tests**
- Inline unit tests (`#[cfg(test)] mod tests`) must cover:
  - `PrinterId::new()` generates unique IDs (two calls produce different values)
  - `Printer::new()` sets initial status to `Offline`
  - `connect()` transitions status to `Online` and emits `PrinterConnected` event
  - `disconnect()` transitions status to `Offline` and emits `PrinterDisconnected` event
  - `set_error()` transitions status to `Error` and emits `PrinterDisconnected` event
  - Business rule: `can_accept_job()` returns `true` only when `Online`
  - Business rule: `can_accept_job()` returns `false` when `Offline` or `Error`
  - `drain_events()` returns events and clears buffer

**AC-7: Test Execution**
- All tests pass with `cargo test`

**AC-8: Domain Independence**
- Domain layer has ZERO external dependencies (no `rusqlite`, `tokio`, `reqwest`, `tracing`, etc.)
- No imports from `infrastructure/`, `application/`, or `interface/` layers
- Only `serde` and `uuid` crates allowed

## Tasks / Subtasks

- [x] **Task 1: Create Domain Error extension** (AC: #3, #8)
  - [x] Add printer-specific error variants to existing `DomainError` in `src-tauri/src/domain/print_job/errors.rs` OR create a separate `src-tauri/src/domain/printer/errors.rs`
  - [x] Required variant: `PrinterNotOnline { printer_name: String }` (used when `can_accept_job()` is false)
  - [x] Re-export from `domain/printer/mod.rs`

- [x] **Task 2: Create Value Objects** (AC: #1, #5)
  - [x] Create `src-tauri/src/domain/printer/value_objects.rs`
  - [x] Implement `PrinterId` as UUID v4 wrapper: `new()`, `Display` impl (same pattern as `JobId`)
  - [x] Implement `PrinterName` as newtype `String` wrapper: `new(name: String) -> Self`, `as_str() -> &str`
  - [x] Implement `PrinterStatus` enum: `Online`, `Offline`, `Error` with derive macros
  - [x] Implement `PrinterType` enum: `Local`, `Network` with derive macros
  - [x] Add unit tests: uniqueness of `PrinterId`, `PrinterName` construction

- [x] **Task 3: Create Domain Events** (AC: #2, #5)
  - [x] Create `src-tauri/src/domain/printer/events.rs`
  - [x] Define `PrinterEvent` trait with `event_type() -> &str` and `aggregate_id() -> &PrinterId`
  - [x] Implement `PrinterConnected { printer_id: PrinterId, timestamp: u64 }`
  - [x] Implement `PrinterDisconnected { printer_id: PrinterId, timestamp: u64 }`
  - [x] Add unit tests for event creation and trait methods

- [x] **Task 4: Create Printer Aggregate** (AC: #3, #5, #6)
  - [x] Create `src-tauri/src/domain/printer/aggregate.rs`
  - [x] Define `Printer` struct with all required fields; use `#[serde(skip)]` on events buffer
  - [x] Implement `Printer::new()`, `connect()`, `disconnect()`, `set_error()`, `can_accept_job()`, `drain_events()`
  - [x] Implement `Clone` manually (clone produces empty events vec, same pattern as `PrintJob`)
  - [x] Add comprehensive unit tests for all business rules (see AC-6)

- [x] **Task 5: Create Repository Trait** (AC: #4)
  - [x] Create `src-tauri/src/domain/printer/repository.rs`
  - [x] Define `PrinterRepository` trait with `save`, `find_all`, `find_by_name` methods
  - [x] Add documentation comments

- [x] **Task 6: Wire Module and Verify** (AC: #7, #8)
  - [x] Update `src-tauri/src/domain/printer/mod.rs` to replace stub comment with submodule declarations
  - [x] Re-export key types: `Printer`, `PrinterId`, `PrinterName`, `PrinterStatus`, `PrinterType`, `PrinterRepository`, `PrinterEvent`
  - [x] Run `cargo build` — must succeed with zero errors
  - [x] Run `cargo test` — all tests must pass
  - [x] Verify domain layer has no imports from other layers
  - [x] Run `cargo clippy` — fix any warnings

## Dev Notes

### 🎯 Story Purpose & Context

Story 1.3 follows the exact same DDD patterns established in Story 1.2 (PrintJob). Story 1.2 completion notes and debug log are the single most valuable reference — read them before coding.

**Key context from Story 1.2:**
- `Box<dyn DomainEvent>` does not implement `Clone` → implement `Clone` manually; the events buffer is transient, so `clone()` returns empty `vec![]`
- `#[serde(skip)]` on events field — events are transient, drained before persistence
- `now_unix()` helper using `SystemTime::now()` — reuse or replicate the same pattern
- `cargo clippy` caught `mark_printing()` missing event and `Display` vs `to_string()` conflicts — apply same care here

**PrintJob references Printer by `PrinterName`** (value object, Decision 3 in architecture.md). `PrinterId` is the stable DB identifier (used in `printer_configs` table, Decision 18). Both must be defined here.

### 🏗️ Architecture Requirements (MANDATORY)

**Domain Layer Rules (NON-NEGOTIABLE):**
1. Domain layer is completely independent — NO dependencies on other layers
2. Only allowed crates: `serde`, `uuid` (already in Cargo.toml)
3. NO `rusqlite`, `tokio`, `reqwest`, `tracing`, or any infrastructure crate
4. NO imports from `infrastructure/`, `application/`, or `interface/`
5. Pure Rust stdlib + serde + uuid only

[Source: architecture.md, Clean Architecture + DDD Pattern]

**Event-Driven Architecture:**
- Every status transition that changes printer availability MUST publish a domain event
- Events are collected in internal buffer, drained AFTER persistence (Outbox Pattern)
- Aggregate itself does NOT publish to external event bus

[Source: architecture.md, Decision 14: Outbox Pattern]

**Decision 18: Printer Aggregate Persistence**
- `PrinterCache` (with 5s TTL) is an **infrastructure** concern — do NOT implement it here
- Domain only defines the `Printer` aggregate and `PrinterRepository` trait
- The Infrastructure layer will implement the caching strategy in Story 2.x

[Source: architecture.md, Decision 18]

### 📁 Exact File Structure Required

```
src-tauri/src/domain/printer/
├── mod.rs              # UPDATE existing stub — add submodule declarations + re-exports
├── value_objects.rs    # PrinterId, PrinterName, PrinterStatus, PrinterType (NEW)
├── events.rs           # PrinterEvent trait + PrinterConnected, PrinterDisconnected (NEW)
├── aggregate.rs        # Printer aggregate root (NEW)
├── repository.rs       # PrinterRepository trait (NEW)
└── errors.rs           # Printer-specific DomainError variants (NEW)
```

**Do NOT touch** `src-tauri/src/domain/print_job/` — it is already complete.
**Do NOT touch** `src-tauri/src/domain/document/` — that is Story 1.4.

### 📦 Type Specifications

```rust
// value_objects.rs
use uuid::Uuid;
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrinterId(Uuid);
impl PrinterId {
    pub fn new() -> Self { Self(Uuid::new_v4()) }
}
impl std::fmt::Display for PrinterId { ... }

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrinterName(String);
impl PrinterName {
    pub fn new(name: String) -> Self { Self(name) }
    pub fn as_str(&self) -> &str { &self.0 }
}
impl std::fmt::Display for PrinterName { ... }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrinterStatus { Online, Offline, Error }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrinterType { Local, Network }
```

```rust
// aggregate.rs
#[derive(Debug, Serialize, Deserialize)]
pub struct Printer {
    id: PrinterId,
    name: PrinterName,
    status: PrinterStatus,
    printer_type: PrinterType,
    #[serde(skip)]
    events: Vec<Box<dyn PrinterEvent>>,
}

impl Clone for Printer {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            name: self.name.clone(),
            status: self.status.clone(),
            printer_type: self.printer_type.clone(),
            events: vec![], // events are transient
        }
    }
}
```

### 🧪 Testing Requirements

Required unit tests (inline `#[cfg(test)] mod tests` in each file):

| Test | Location | Assertion |
|------|----------|-----------|
| `test_printer_id_unique` | value_objects.rs | Two `PrinterId::new()` calls differ |
| `test_printer_new_status_is_offline` | aggregate.rs | `Printer::new()` → status `Offline` |
| `test_connect_transitions_to_online` | aggregate.rs | `connect()` → status `Online` |
| `test_connect_emits_event` | aggregate.rs | `drain_events()` after `connect()` returns 1 `PrinterConnected` |
| `test_disconnect_transitions_to_offline` | aggregate.rs | `disconnect()` → status `Offline` |
| `test_disconnect_emits_event` | aggregate.rs | `drain_events()` after `disconnect()` returns 1 `PrinterDisconnected` |
| `test_set_error_transitions_to_error` | aggregate.rs | `set_error()` → status `Error` |
| `test_set_error_emits_disconnected_event` | aggregate.rs | `drain_events()` after `set_error()` returns 1 `PrinterDisconnected` |
| `test_can_accept_job_online` | aggregate.rs | `can_accept_job()` → `true` when `Online` |
| `test_cannot_accept_job_offline` | aggregate.rs | `can_accept_job()` → `false` when `Offline` |
| `test_cannot_accept_job_error` | aggregate.rs | `can_accept_job()` → `false` when `Error` |
| `test_drain_events_clears_buffer` | aggregate.rs | Second `drain_events()` returns empty vec |

### 🔒 Anti-Patterns (DO NOT)

1. ❌ DO NOT import from other layers
2. ❌ DO NOT add new crate dependencies
3. ❌ DO NOT implement `PrinterRepository` — only define the trait
4. ❌ DO NOT implement `PrinterCache` — that belongs to Infrastructure (Story 2.x)
5. ❌ DO NOT use async
6. ❌ DO NOT use `println!` or `tracing`
7. ❌ DO NOT create separate test directories
8. ❌ DO NOT modify `src-tauri/src/domain/print_job/` or `domain/document/`

### 📚 References

- Story 1.2 implementation patterns: `_bmad-output/implementation-artifacts/1-2-create-printjob-domain-model.md`
- Architecture decisions 3, 5, 14, 18: `_bmad-output/planning-artifacts/architecture.md`
- Epic 1 requirements: `_bmad-output/planning-artifacts/epics.md` (line ~433)

## Dev Agent Record

### Agent Model Used

Kiro (Claude) — Amelia persona

### Debug Log References

No issues encountered. Patterns from Story 1.2 applied directly:
- Manual `Clone` impl with empty events vec ✅
- `#[serde(skip)]` on events field ✅
- `now_unix()` helper replicated in events.rs ✅
- `Display` impl instead of `to_string()` ✅

### Completion Notes List

✅ **All 6 Tasks Completed Successfully**

- Task 1: `printer/errors.rs` — `DomainError::PrinterNotOnline` + 2 unit tests
- Task 2: `printer/value_objects.rs` — `PrinterId`, `PrinterName`, `PrinterStatus`, `PrinterType` + 7 unit tests
- Task 3: `printer/events.rs` — `PrinterEvent` trait, `PrinterConnected`, `PrinterDisconnected` + 3 unit tests
- Task 4: `printer/aggregate.rs` — `Printer` aggregate, 5 methods, manual `Clone` + 13 unit tests
- Task 5: `printer/repository.rs` — `PrinterRepository` trait with 3 methods
- Task 6: `printer/mod.rs` wired; `cargo build` 0 errors; `cargo test` 80 passed (25 new + 55 prior); `cargo clippy` 0 warnings

Domain layer remains independent: zero imports from infrastructure, application, or interface.

### File List

**New files:**
- `src-tauri/src/domain/printer/errors.rs`
- `src-tauri/src/domain/printer/value_objects.rs`
- `src-tauri/src/domain/printer/events.rs`
- `src-tauri/src/domain/printer/aggregate.rs`
- `src-tauri/src/domain/printer/repository.rs`

**Modified files:**
- `src-tauri/src/domain/printer/mod.rs`

### Review Findings

- [x] [Review][Decision] `DomainError` naming conflict — renamed `DomainError` → `PrinterDomainError` trong `printer/errors.rs`. Updated `aggregate.rs`, `repository.rs`, `mod.rs`. `cargo build` + `cargo test` (80/80) clean.
- [x] [Review][Patch] `printer::DomainError` thiếu derive `Clone` — `Clone` đã có trong `PrinterDomainError` (applied khi rename). [`src-tauri/src/domain/printer/errors.rs:4`]
- [x] [Review][Patch] `connect()`/`disconnect()` không guard state transition — thêm doc comment rõ behavior idempotency tại `connect()` và `disconnect()`. [`src-tauri/src/domain/printer/aggregate.rs:72`]
- [x] [Review][Defer] `FromStr` impl không được spec yêu cầu, không có test cover [`src-tauri/src/domain/printer/value_objects.rs:28`] — deferred, pre-existing

#### Round 2 Review (2026-06-22)

- [x] [Review][Patch] `now_unix()` panic khi system clock trước UNIX_EPOCH — `.unwrap()` → `.unwrap_or_default()` [`src-tauri/src/domain/printer/events.rs:7`]
- [x] [Review][Patch] `connect()` không có idempotency guard — emits duplicate `PrinterConnected` khi đã `Online` — thêm early return guard [`src-tauri/src/domain/printer/aggregate.rs:connect`]
- [x] [Review][Patch] `disconnect()` không có idempotency guard — emits duplicate `PrinterDisconnected` khi đã `Offline` — thêm early return guard [`src-tauri/src/domain/printer/aggregate.rs:disconnect`]
- [x] [Review][Patch] `set_error()` không có idempotency guard — emits duplicate `PrinterDisconnected` khi đã `Error` — thêm early return guard [`src-tauri/src/domain/printer/aggregate.rs:set_error`]
- [x] [Review][Defer] `set_error()` emit `PrinterDisconnected` thay vì `PrinterErrored` — deferred, per-spec AC-2 chỉ định 2 event types; consumer dùng `status()` để phân biệt
- [x] [Review][Defer] `PrinterName::new` không validate empty string — deferred, không phải domain rule per spec; upstream validate
- [x] [Review][Defer] `PrinterRepository::save` nhận `&Printer` thay vì consume events — deferred, intentional pattern (Story 1.2), drain ở application layer
- [x] [Review][Defer] `PrinterRepository` thiếu `find_by_id` — deferred, không trong spec 1.3; Story 2.x

## Change Log

**2026-06-22: Story 1.3 Implementation Complete**
- Implemented complete Printer domain model following Story 1.2 patterns
- 25 new unit tests; 80 total passing (0 regressions)
- Domain layer remains independent (zero external dependencies)
- `cargo build`, `cargo test`, `cargo clippy` all clean
- Status: ✅ ALL Acceptance Criteria met — Ready for review
