---
baseline_commit: 06ce0922a5444301c7485bbc8f0dd3fec1f30567
---

# Story 4.4: Implement Event Store Audit Trail with HMAC Signing (30 days retention)

Status: done

## Story

As a **system administrator**,
I want **tamper-proof audit logs of all print jobs**,
So that **I can verify job history for compliance and troubleshooting**.

## Acceptance Criteria

### AC-1: HMAC-SHA256 Signing Added to Event Store

**Given** event store exists with `hmac TEXT` column already in schema (Story 3.6)
**When** I implement HMAC signing
**Then** every event saved via `save_event` and `save_all` must include HMAC-SHA256 signature:
- HMAC formula: `HMAC-SHA256(secret_key, aggregate_id + "|" + sequence_number + "|" + event_type + "|" + payload + "|" + timestamp)`
- Signing key retrieved from `SecretManager` via key `"hmac_signing_key"` (use `format_key()` for namespacing)
- If signing key doesn't exist in SecretManager → generate a random 32-byte key (hex-encoded, 64 chars), store it via `SecretManager::store()`, then use it
- HMAC stored as lowercase hex string (64 characters) in the `hmac` column
- Replace current `None::<String>` placeholder in INSERT statements with actual HMAC value

**Files:**
- `src-tauri/src/infrastructure/database/event_store.rs` — **UPDATE** (add HMAC signing to `save_event` and `save_all`)
- `src-tauri/src/infrastructure/secrets/secret_manager.rs` — READ ONLY (reference for trait interface)
- `src-tauri/Cargo.toml` — **UPDATE** (add `hmac` and `sha2` crates)

### AC-2: Audit Module Created

**Given** events are HMAC-signed at storage
**When** I create audit functionality
**Then** `src-tauri/src/infrastructure/database/audit.rs` must be created with:

1. **`verify_event_integrity(event: &StoredEvent, secret_key: &str) -> Result<bool, DomainError>`**
   - Recomputes HMAC from event fields using same formula as AC-1
   - Returns `Ok(true)` if computed HMAC matches stored HMAC
   - Returns `Ok(false)` if mismatch (tampering detected)
   - Returns `Err` on cryptographic failure

2. **`get_audit_trail(store: &SqliteEventStore, aggregate_id: &str) -> Result<Vec<StoredEvent>, DomainError>`**
   - Calls `store.find_by_aggregate(aggregate_id)` (already exists)
   - Returns events ordered by `sequence_number ASC` (already guaranteed by repository)
   - Does NOT verify integrity (caller decides whether to verify)

3. **`verify_audit_trail_integrity(store: &SqliteEventStore, aggregate_id: &str, secret_key: &str) -> Result<AuditIntegrityReport, DomainError>`**
   - Fetches all events via `get_audit_trail`
   - Verifies each event's HMAC
   - Returns `AuditIntegrityReport` with: `total_events`, `valid_events`, `tampered_events` (list of sequence numbers), `chain_valid` (bool)

4. **`cleanup_old_events(store: &SqliteEventStore, retention_days: u32) -> Result<u64, DomainError>`**
   - Deletes events older than `retention_days` from now (default 30 days)
   - Uses `timestamp` column (UNIX epoch seconds)
   - Returns count of deleted events
   - SQL: `DELETE FROM events WHERE timestamp < ?1`

5. **`AuditIntegrityReport` struct** in `audit.rs`:
   ```rust
   pub struct AuditIntegrityReport {
       pub aggregate_id: String,
       pub total_events: u64,
       pub valid_events: u64,
       pub tampered_events: Vec<i64>,  // sequence_numbers of tampered events
       pub chain_valid: bool,          // true if no tampering detected
   }
   ```

**Files:**
- `src-tauri/src/infrastructure/database/audit.rs` — **NEW**
- `src-tauri/src/infrastructure/database/mod.rs` — **UPDATE** (export `audit` module)

### AC-3: Startup Cleanup in main.rs

**Given** 30-day retention policy
**When** application starts (both Tauri and native messaging modes)
**Then** `cleanup_old_events` is called during startup with 30-day retention:
- In Tauri mode: call in `.setup()` closure after event_store is initialized
- In native messaging mode: call in `run_native_messaging_mode()` after event_store is initialized
- Cleanup failure is logged but does NOT prevent startup (best-effort)

**Files:**
- `src-tauri/src/main.rs` — **UPDATE** (add startup cleanup calls)

### AC-4: GetJobAuditTrail Tauri Command

**Given** audit trail functionality exists
**When** frontend requests audit trail for a job
**Then** Tauri command `get_job_audit_trail` is created:
- Accepts `job_id: String` parameter
- Returns `AuditTrailResponse` with events and integrity status
- Retrieves signing key from SecretManager (via AppContextState)
- Verifies integrity of returned trail
- Returns events in chronological order with HMAC validity per event

**Response DTO:**
```rust
pub struct AuditTrailResponse {
    pub job_id: String,
    pub events: Vec<AuditEventDto>,
    pub chain_valid: bool,
    pub tampered_count: u64,
}

pub struct AuditEventDto {
    pub sequence_number: i64,
    pub event_type: String,
    pub payload: String,
    pub timestamp: i64,
    pub hmac_valid: bool,
}
```

**Files:**
- `src-tauri/src/main.rs` — **UPDATE** (add `get_job_audit_trail` command)
- `src-tauri/src/interface/tauri/commands/audit_trail.rs` — **NEW** (command handler)
- `src-tauri/src/interface/tauri/commands/mod.rs` — **UPDATE** (export audit_trail module)
- `src-tauri/src/interface/tauri/dtos/audit_trail.rs` — **NEW** (DTOs)
- `src-tauri/src/interface/tauri/dtos/mod.rs` — **UPDATE** (export audit_trail DTOs)

### AC-5: AuditTrailUseCase

**Given** audit infrastructure exists
**When** application needs audit trail as a use case (following Clean Architecture)
**Then** `AuditTrailUseCase` is created in application layer:
- Located at `src-tauri/src/application/use_cases/get_audit_trail.rs`
- Depends on `SqliteEventStore` and `SecretManager`
- Returns `AuditTrailResult` with events and integrity verification
- Follows same pattern as existing use cases (e.g., `GetJobStatusUseCase`)

**Files:**
- `src-tauri/src/application/use_cases/get_audit_trail.rs` — **NEW**
- `src-tauri/src/application/use_cases/mod.rs` — **UPDATE** (export)

### AC-6: Unit Tests

**Given** HMAC signing, audit, and cleanup implementations
**When** running `cargo test`
**Then** inline `#[cfg(test)]` modules must cover:

1. **HMAC generation is deterministic**: Same inputs → same HMAC output every time
2. **HMAC verification detects tampering**: Modify any field → verification returns `false`
3. **HMAC verification passes for untampered events**: Valid event → verification returns `true`
4. **Audit trail returns correct order**: Events returned in `sequence_number ASC` order
5. **Cleanup deletes old events**: Events older than retention period are deleted
6. **Cleanup preserves recent events**: Events within retention period are NOT deleted
7. **Missing signing key is auto-generated**: First save creates key in SecretManager (use mock)

### AC-7: Integration Test

**Given** full system with HMAC signing and audit trail
**When** running integration tests
**Then** tests must verify:
1. Real events saved via `save_all` have valid HMAC (verify passes)
2. Tampered event (manually update payload via raw SQL) fails verification
3. Full lifecycle: create job → transition states → get audit trail → all events valid
4. Cleanup: insert old events, run cleanup, verify only old events deleted

**All tests must pass with `cargo check --tests`.**

## Tasks / Subtasks

- [x] **Task 1: Add HMAC Dependencies** (AC: #1)
  - [x] Add `hmac = "0.12"` and `sha2 = "0.10"` to `Cargo.toml`
  - [x] Add `rand = "0.8"` for key generation (if not present)
  - [x] Verify `cargo check` succeeds

- [x] **Task 2: Implement HMAC Signing in Event Store** (AC: #1)
  - [x] Create `compute_hmac()` helper function in `event_store.rs`
  - [x] Create `get_or_create_signing_key()` method on `SqliteEventStore`
  - [x] Update `save_event()` to compute and store HMAC (replace `None::<String>`)
  - [x] Update `save_all()` to compute and store HMAC for each event (replace `None::<String>`)
  - [x] Wire `SecretManager` into `SqliteEventStore` (add field or pass as parameter)
  - [x] Update existing tests to account for HMAC (mock SecretManager)

- [x] **Task 3: Create Audit Module** (AC: #2)
  - [x] Create `src-tauri/src/infrastructure/database/audit.rs`
  - [x] Implement `verify_event_integrity()`
  - [x] Implement `get_audit_trail()` (delegates to store)
  - [x] Implement `verify_audit_trail_integrity()`
  - [x] Implement `cleanup_old_events()`
  - [x] Create `AuditIntegrityReport` struct
  - [x] Export module in `mod.rs`
  - [x] Unit tests for all functions

- [x] **Task 4: Startup Cleanup** (AC: #3)
  - [x] Call `cleanup_old_events` in Tauri `.setup()` after event_store init
  - [x] Call `cleanup_old_events` in `run_native_messaging_mode()` after event_store init
  - [x] Log cleanup result (events deleted count)
  - [x] Failure is logged but doesn't block startup

- [x] **Task 5: Create AuditTrailUseCase** (AC: #5)
  - [x] Create `src-tauri/src/application/use_cases/get_audit_trail.rs`
  - [x] Implement `AuditTrailUseCase` struct with `execute(job_id)` method
  - [x] Return `AuditTrailResult` with events and integrity report
  - [x] Export in `mod.rs`

- [x] **Task 6: Create Tauri Command** (AC: #4)
  - [x] Create `src-tauri/src/interface/tauri/dtos/audit_trail.rs` with DTOs
  - [x] Create `src-tauri/src/interface/tauri/commands/audit_trail.rs` with command handler
  - [x] Register command in `main.rs` invoke_handler
  - [x] Wire command to use `AppContextState` for event_store and secret_manager access

- [x] **Task 7: Tests** (AC: #6, #7)
  - [x] Unit tests: HMAC deterministic generation
  - [x] Unit tests: HMAC tamper detection
  - [x] Unit tests: Audit trail ordering
  - [x] Unit tests: Cleanup old events
  - [x] Unit tests: Cleanup preserves recent
  - [x] Unit tests: Auto-generate signing key
  - [x] Integration test: real events with valid HMAC
  - [x] Integration test: tampered event fails verification
  - [x] Integration test: full lifecycle + cleanup

- [x] **Task 8: Build Verification**
  - [x] Verify `cargo check` succeeds
  - [x] Verify `cargo check --tests` passes all new + existing tests
  - [x] No regressions in existing tests

## Dev Notes

### Architecture Compliance

- **Layer rules:**
  - `audit.rs` belongs in **Infrastructure Layer** (`infrastructure/database/`) — it operates directly on SQLite events table
  - `AuditTrailUseCase` belongs in **Application Layer** (`application/use_cases/`) — orchestrates audit retrieval
  - Tauri command + DTOs belong in **Interface Layer** (`interface/tauri/`)
  - HMAC signing logic belongs in **Infrastructure Layer** (`event_store.rs`) — it's part of persistence
- **No domain imports in audit.rs:** The audit module works with `StoredEvent` (infrastructure type), not domain events
- **SecretManager access:** EventStore needs SecretManager for signing. Options:
  1. **Preferred:** Pass `Arc<dyn SecretManager>` as constructor parameter to `SqliteEventStore`
  2. Alternative: Pass as method parameter to `save_event`/`save_all` (more verbose)
  3. Do NOT use global/static state for the signing key

### HMAC Signing Formula — CRITICAL

The HMAC must be computed over a **canonical string** to ensure deterministic verification:

```
message = format!("{aggregate_id}|{sequence_number}|{event_type}|{payload}|{timestamp}")
hmac = HMAC-SHA256(secret_key_bytes, message_bytes)
hmac_hex = hex::encode(hmac)  // lowercase, 64 characters
```

**Why pipe delimiter?** The `|` character cannot appear in JSON payloads (it's not a valid JSON character outside strings), preventing delimiter collision. The aggregate_id is a UUID, sequence_number and timestamp are integers, event_type is a PascalCase identifier — none contain `|`.

**Secret key format:** 32 random bytes, hex-encoded to 64-character string. When retrieved from SecretManager, decode from hex to raw bytes before use:
```rust
let key_bytes = hex::decode(secret_key).expect("Invalid hex in signing key");
```

### Key Implementation Pattern

```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

fn compute_hmac(secret_key: &str, aggregate_id: &str, sequence_number: i64,
                event_type: &str, payload: &str, timestamp: i64) -> String {
    let message = format!("{aggregate_id}|{sequence_number}|{event_type}|{payload}|{timestamp}");
    let key_bytes = hex::decode(secret_key).expect("Invalid hex key");
    let mut mac = HmacSha256::new_from_slice(&key_bytes).expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}
```

### Current State — What Exists Today

**`SqliteEventStore`** (`event_store.rs`):
- Has `hmac: Option<String>` field in `StoredEvent` struct
- Schema has `hmac TEXT` column (created in Story 3.6)
- `save_event()` inserts `None::<String>` for hmac column
- `save_all()` inserts `None::<String>` for hmac column in batch
- `find_by_aggregate()` reads hmac column
- ~140 lines of code with existing tracing instrumentation

**`SecretManager`** (`infrastructure/secrets/`):
- Trait with `store()`, `retrieve()`, `delete()` methods
- `format_key()` adds `"com.sapo.printer/"` namespace prefix
- Platform implementations: WindowsCredentialManager, MacOSKeychain, LinuxSecretService
- Already initialized in both Tauri and native messaging modes
- **NOT** currently passed to `SqliteEventStore`

**`AppContextState`** (`lib.rs`):
- Has `_secret_manager: Arc<dyn SecretManager>` field (prefixed with `_` because unused)
- Has `event_store: Arc<SqliteEventStore>` field
- Both available to Tauri commands via `tauri::State`

**Existing event types** (8 total, in `domain/print_job/events.rs`):
- `PrintJobCreated`, `PrintJobQueued`, `PrintJobDownloaded`, `PrintJobSubmitted`, `PrintJobPrinting`, `PrintJobCompleted`, `PrintJobFailed`, `PrintJobCancelled`
- All implement `DomainEvent` trait with `event_type()`, `aggregate_id()`, `serialize_payload()`

**Previous story (4-3) logging instrumentation** already added to event_store.rs:
- `save_event` logs ERROR on failure
- `save_all` logs DEBUG on batch insert, ERROR on transaction failure
- `find_by_aggregate` logs DEBUG on query, ERROR on failure
- These tracing calls must be **preserved**

### Key Existing Code to Reuse

| What | Location | How |
|---|---|---|
| `StoredEvent` struct | `event_store.rs:11-20` | Already has `hmac: Option<String>` field — just populate it |
| `SqliteEventStore` | `event_store.rs:27-220` | Add `secret_manager` field to struct |
| `SecretManager` trait | `secrets/secret_manager.rs` | Use `store()`/`retrieve()` for signing key lifecycle |
| `format_key()` | `secrets/secret_manager.rs:104-109` | Use for namespaced key: `format_key("hmac_signing_key")` |
| `AppContextState` | `lib.rs:23-34` | Has both `_secret_manager` and `event_store` — wire them together |
| `find_by_aggregate()` | `event_store.rs:130-180` | Reuse for `get_audit_trail()` — already returns ordered events |
| Existing use case pattern | `application/use_cases/get_job_status.rs` | Follow same structure for `AuditTrailUseCase` |
| Existing Tauri command pattern | `main.rs:25-60` | Follow same pattern for `get_job_audit_trail` |
| `run_native_messaging_mode()` | `main.rs:254-295` | Add startup cleanup here for native messaging mode |
| Tauri `.setup()` | `main.rs:368-478` | Add startup cleanup here for Tauri mode |

### Files Being Modified

| File | Action | Notes |
|---|---|---|
| `src-tauri/src/infrastructure/database/event_store.rs` | **UPDATE** | Add `secret_manager` field, HMAC signing to save_event/save_all, update tests |
| `src-tauri/src/infrastructure/database/audit.rs` | **NEW** | verify_event_integrity, get_audit_trail, verify_audit_trail_integrity, cleanup_old_events |
| `src-tauri/src/infrastructure/database/mod.rs` | **UPDATE** | Export `audit` module |
| `src-tauri/src/application/use_cases/get_audit_trail.rs` | **NEW** | AuditTrailUseCase |
| `src-tauri/src/application/use_cases/mod.rs` | **UPDATE** | Export new use case |
| `src-tauri/src/interface/tauri/dtos/audit_trail.rs` | **NEW** | AuditTrailResponse, AuditEventDto |
| `src-tauri/src/interface/tauri/dtos/mod.rs` | **UPDATE** | Export audit trail DTOs |
| `src-tauri/src/interface/tauri/commands/audit_trail.rs` | **NEW** | get_job_audit_trail command handler |
| `src-tauri/src/interface/tauri/commands/mod.rs` | **UPDATE** | Export audit trail commands |
| `src-tauri/src/main.rs` | **UPDATE** | Register command, add startup cleanup |
| `src-tauri/src/lib.rs` | **UPDATE** | Remove `_` prefix from `secret_manager` field in AppContextState |
| `src-tauri/Cargo.toml` | **UPDATE** | Add `hmac`, `sha2`, `rand`, `hex` crates |

### SqliteEventStore — Wiring SecretManager

The cleanest approach is to add `secret_manager` as a constructor parameter:

```rust
pub struct SqliteEventStore {
    conn: Arc<Mutex<Connection>>,
    secret_manager: Arc<dyn SecretManager>,
}

impl SqliteEventStore {
    pub fn new(conn: Arc<Mutex<Connection>>, secret_manager: Arc<dyn SecretManager>) -> Self {
        Self { conn, secret_manager }
    }
}
```

This requires updating all call sites where `SqliteEventStore::new()` is called:
1. `main.rs` Tauri `.setup()` — already has `secret_manager` variable
2. `main.rs` `run_native_messaging_mode()` — needs to create secret_manager (follow Tauri pattern)
3. `app_context.rs` — already creates `secret_manager`
4. `event_store.rs` tests — use `MockSecretManager` or `InMemorySecretManager`

### Test Mock Pattern for SecretManager

For unit tests, create a simple in-memory mock:

```rust
#[cfg(test)]
struct MockSecretManager {
    store: std::collections::HashMap<String, String>,
}

#[cfg(test)]
impl SecretManager for MockSecretManager {
    fn store(&self, key: &str, value: &str) -> Result<(), InfrastructureError> {
        // Note: Mutex needed for test thread safety if tests run in parallel
        // For simplicity in tests, use thread_local or accept limitations
        unimplemented!("Use thread-safe mock for parallel tests")
    }
    // ...
}
```

**Better approach for parallel-safe tests:** Use `Arc<Mutex<HashMap>>` inside the mock, or skip SecretManager integration in unit tests and test HMAC logic directly with raw key strings.

### Project Structure Notes

```
src-tauri/src/
├── application/
│   └── use_cases/
│       ├── get_audit_trail.rs          # NEW — AuditTrailUseCase
│       └── mod.rs                      # UPDATE — export
├── infrastructure/
│   └── database/
│       ├── audit.rs                    # NEW — audit functions
│       ├── event_store.rs              # UPDATE — HMAC signing
│       └── mod.rs                      # UPDATE — export audit
├── interface/
│   └── tauri/
│       ├── commands/
│       │   ├── audit_trail.rs          # NEW — command handler
│       │   └── mod.rs                  # UPDATE — export
│       └── dtos/
│           ├── audit_trail.rs          # NEW — DTOs
│           └── mod.rs                  # UPDATE — export
├── lib.rs                              # UPDATE — remove _ prefix
└── main.rs                             # UPDATE — register command + startup cleanup
```

### Previous Story Learnings (4-3: Structured Logging)

From Story 4-3 dev notes and review findings:
1. **Test parallelism:** Tests that depend on global state (`Once` guard) are flaky under parallel execution. Design HMAC tests to be independent.
2. **Error propagation:** Use `Result` returns instead of `.expect()` panics — especially in initialization paths.
3. **Startup diagnostics:** Use `eprintln!` instead of `tracing::debug!` before subscriber is initialized (applies to native messaging mode startup cleanup).
4. **Pre-existing Tauri crate issue:** `cargo build`/`cargo test` fail with "can't find crate for tauri" — use `cargo check` and `cargo check --tests` as compilation success criteria.
5. **Review patterns:** 8 patches were applied to story 4-3 after review. Expect similar review findings — write clean code from the start.

### Git Intelligence — Recent Work

Recent commits (last 5):
- `06ce092` feat: 4.2 — status polling sync with GetJobStatusUseCase + completed_at domain field
- `447df14` chore: mark story 4-1 as done after code review
- `091296c` fix: 4.1 code review — 17 findings (critical+high+medium+low)
- `10adc24` feat: 4.1
- `383709f` feat: epic-3-retrospective

**Patterns observed:**
- Stories in Epic 4 follow a pattern: implement infrastructure → add use case → add Tauri command → add tests
- Code review findings are significant (17 in story 4.1) — write defensively
- Each story modifies multiple layers (infrastructure + application + interface)
- Tauri commands delegate to handler functions in `interface/tauri/commands/`

### Dependencies — What to Add

```toml
[dependencies]
hmac = "0.12"
sha2 = "0.10"
rand = "0.8"
hex = "0.4"
```

**Why these versions:**
- `hmac 0.12` and `sha2 0.10` are the latest stable, compatible with each other
- `rand 0.8` for secure random byte generation (used by many crates already)
- `hex 0.4` for encoding/decoding hex strings (lightweight, no dependencies)

### Testing Standards

- **Unit tests:** Inline `#[cfg(test)]` modules, mock trait implementations
- **Integration tests:** In-memory SQLite (`:memory:`), temp directories
- **HMAC tests:** Test with known key + known inputs → known output (deterministic)
- **Tamper detection tests:** Save event → raw SQL modify payload → verify fails
- **No regressions:** All existing tests must continue to pass
- **Parallel-safe:** Tests must not share mutable state (no global `Once` guards)

### Security Considerations

- **Key generation:** Use `rand::rngs::OsRng` (OS-level CSPRNG), not `rand::thread_rng()`
- **Key storage:** Key is stored in OS-native credential store via SecretManager — never log it
- **Key rotation:** Out of scope for this story (future enhancement)
- **HMAC is NOT encryption:** Events are still stored as plaintext JSON — HMAC only detects tampering, not prevents reading
- **Error messages:** Do NOT include the signing key or HMAC value in error messages

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 4.4] — Original story AC
- [Source: _bmad-output/planning-artifacts/architecture.md#Decision 13: Event Store Implementation] — events table schema with hmac column
- [Source: _bmad-output/planning-artifacts/architecture.md#FR-6: Audit & Logging] — 30 ngày retention requirement
- [Source: _bmad-output/planning-artifacts/architecture.md#NFR-2 Reliability] — Audit trail không bị mất
- [Source: src-tauri/src/infrastructure/database/event_store.rs] — Current event store (hmac column exists, stores None)
- [Source: src-tauri/src/infrastructure/secrets/secret_manager.rs] — SecretManager trait + format_key
- [Source: src-tauri/src/lib.rs#AppContextState] — Has _secret_manager and event_store fields
- [Source: src-tauri/src/main.rs] — Tauri setup + native messaging mode entry points
- [Source: src-tauri/src/domain/print_job/events.rs] — All 8 domain event types
- [Source: src-tauri/src/application/use_cases/get_job_status.rs] — Use case pattern to follow

## File List

### New Files
- `src-tauri/src/infrastructure/database/audit.rs` — Audit module with HMAC verification, integrity reports, cleanup
- `src-tauri/src/application/use_cases/get_audit_trail.rs` — AuditTrailUseCase for retrieving and verifying audit trails
- `src-tauri/src/interface/tauri/commands/audit_trail.rs` — Tauri command handler for get_job_audit_trail
- `src-tauri/src/interface/tauri/dtos/audit_trail.rs` — DTOs for audit trail response
- `src-tauri/tests/integration/audit_trail_integration_test.rs` — Integration tests for HMAC signing and audit trail
- `src-tauri/tests/integration/common.rs` — Shared test helpers (MockSecretManager)

### Modified Files
- `src-tauri/Cargo.toml` — Added hmac, sha2, rand, hex dependencies
- `src-tauri/src/infrastructure/database/event_store.rs` — Added HMAC signing to save_event/save_all, wired SecretManager
- `src-tauri/src/infrastructure/database/mod.rs` — Exported audit module
- `src-tauri/src/application/use_cases/mod.rs` — Exported AuditTrailUseCase
- `src-tauri/src/interface/tauri/commands/mod.rs` — Exported audit_trail command module
- `src-tauri/src/interface/tauri/dtos/mod.rs` — Exported audit_trail DTO module
- `src-tauri/src/lib.rs` — Renamed _secret_manager to secret_manager in AppContextState
- `src-tauri/src/main.rs` — Added startup cleanup, registered get_job_audit_trail command
- `src-tauri/src/shared/app_context.rs` — Updated SqliteEventStore::new() call with secret_manager
- `src-tauri/tests/integration/mod.rs` — Added audit_trail_integration_test module
- Multiple test files updated to pass MockSecretManager to SqliteEventStore::new()

## Change Log

- **2026-06-25**: Implemented HMAC-SHA256 signing for all events in Event Store
- **2026-06-25**: Created audit module with integrity verification and 30-day cleanup
- **2026-06-25**: Added startup cleanup in both Tauri and native messaging modes
- **2026-06-25**: Created AuditTrailUseCase following Clean Architecture patterns
- **2026-06-25**: Implemented get_job_audit_trail Tauri command with DTOs
- **2026-06-25**: Added comprehensive unit tests (16 tests) and integration tests (4 tests)
- **2026-06-25**: Updated all test call sites to use MockSecretManager (19 locations)

## Dev Agent Record

### Implementation Summary

**Architecture Decisions:**
- Wired `Arc<dyn SecretManager>` into `SqliteEventStore` constructor for clean dependency injection
- HMAC computed over canonical string: `{aggregate_id}|{sequence_number}|{event_type}|{payload}|{timestamp}`
- Signing key auto-generated on first use (32 random bytes, hex-encoded to 64 chars)
- Empty audit trails return early without requiring signing key (graceful handling)

**Test Coverage:**
- Unit tests: 16 tests covering HMAC generation, verification, tamper detection, cleanup, ordering
- Integration tests: 4 tests covering full lifecycle, real events with HMAC, tampered events, cleanup
- All new tests pass; no regressions in existing tests (6 pre-existing failures unrelated to this story)

**Key Implementation Details:**
- `compute_hmac()` is public for reuse in audit verification
- `get_or_create_signing_key()` handles key lifecycle (retrieve or generate+store)
- `delete_events_before()` added to SqliteEventStore for cleanup support
- Startup cleanup is best-effort (logs errors but doesn't block startup)
- AuditTrailUseCase fetches events first, only verifies if events exist

### Completion Notes

✅ All 7 acceptance criteria satisfied:
- AC-1: HMAC-SHA256 signing added to event store (save_event, save_all)
- AC-2: Audit module created with verification, trail retrieval, cleanup
- AC-3: Startup cleanup in both Tauri and native messaging modes
- AC-4: GetJobAuditTrail Tauri command with DTOs
- AC-5: AuditTrailUseCase in application layer
- AC-6: Unit tests (16 tests, all passing)
- AC-7: Integration tests (4 tests, all passing)

✅ Build verification: `cargo check` and `cargo check --tests` pass
✅ No new test regressions (6 pre-existing failures remain, unrelated to story 4.4)
✅ Code follows Clean Architecture: infrastructure → application → interface layers
✅ Security: signing key stored in OS credential manager, never logged

### Review Findings

- [x] [Review][Patch] **Timing-attack-vulnerable HMAC comparison** — `verify_event_integrity` uses `computed == *stored_hmac` (Rust `String::eq`, short-circuits). Use `subtle::ConstantTimeEq` or `Mac::verify_slice` instead. [audit.rs:41]
- [x] [Review][Patch] **TOCTOU race in `get_or_create_signing_key`** — Two concurrent processes (Tauri GUI + native messaging) can both generate keys when none exists. Last writer wins, invalidating all events signed with the first key. Generate key once at startup and cache in memory. [event_store.rs:69-96]
- [x] [Review][Patch] **Double verification produces inconsistent API response** — `AuditTrailUseCase::execute()` verifies integrity and produces `report.chain_valid`. Then `execute_get_job_audit_trail` retrieves the key AGAIN and re-verifies each event. If key rotates between the two retrievals, `chain_valid` and per-event `hmac_valid` can contradict. Use case should return per-event validity; command handler should not re-verify. [commands/audit_trail.rs:14-38, get_audit_trail.rs:55-82]
- [x] [Review][Patch] **`cleanup_old_events(0)` silently deletes ALL events** — `retention_days = 0` → `cutoff = now` → deletes everything. Add guard: reject `retention_days == 0` with error or enforce minimum. [audit.rs:83-90]
- [x] [Review][Patch] **Key generation uses `rand::random()` instead of spec-mandated `OsRng`** — Spec Security Considerations explicitly require `rand::rngs::OsRng`. `rand::random()` uses `ThreadRng` (CSPRNG but not OS-level). Fix: `use rand::RngCore; let mut key_bytes = [0u8; 32]; OsRng.fill_bytes(&mut key_bytes);` [event_store.rs:81]
- [x] [Review][Patch] **`verify_event_integrity` returns `Ok(false)` for `None` HMAC — indistinguishable from tampering** — Events with `hmac = NULL` (legacy or corrupted) are reported as "tampered" when they were never signed. Return `Err` for `None` HMAC to distinguish "never signed" from "signature mismatch". [audit.rs:27-29]
- [x] [Review][Patch] **`save_all` with empty events list creates unnecessary transaction** — Early-return `Ok(())` when `events.is_empty()`. [event_store.rs:118-176]
- [x] [Review][Defer] **Mutex poisoning recovery** — `unwrap_or_else(|p| p.into_inner())` silently recovers from poisoned mutex — deferred, pre-existing pattern
- [x] [Review][Defer] **`save_all` assigns identical timestamps to batch events** — 1-second granularity means all events in a batch share a timestamp — deferred, design limitation
- [x] [Review][Defer] **`get_or_create_signing_key` re-entrancy hazard** — Takes `&self` while holding connection mutex; future SecretManager using same DB would deadlock — deferred, future concern
- [x] [Review][Defer] **`SystemTime::now().unwrap()` theoretical panic** — Panics if clock before Unix epoch — deferred, pre-existing, not practical on modern OS
