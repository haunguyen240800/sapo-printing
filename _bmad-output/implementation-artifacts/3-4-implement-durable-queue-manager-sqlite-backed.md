# Story 3.4: Implement Durable Queue Manager (SQLite-backed)

Status: ready-for-dev

## Story

As a **developer**,
I want **a durable queue backed by SQLite**,
So that **jobs persist across app restarts and aren't lost on crash**.

## Context

Story này implement `QueueManager` — layer trung gian giữa `CreatePrintJobUseCase` (Story 3.3) và `QueueWorker` (Story 3.5). QueueManager dùng `print_jobs` table đã có làm backing store, không cần schema mới.

**Foundation đã có:**
- `print_jobs` table với `status`, `created_at`, `updated_at` columns ✅
- `SqlitePrintJobRepository` với `save`, `update`, `find_by_status` ✅
- `PrintJob::queue()` → PENDING→QUEUED, `PrintJob::reconstruct()` ✅
- `PrintStatus` variants: Pending, Queued, Downloaded, SubmittedToQueue, Printing, Completed, Failed, Cancelled ✅
- `status_to_string` dùng `{:?}` format → "Pending", "Queued", v.v. ✅
- `AppContextState` trong `main.rs` — story này phải extend thêm `queue_manager` ✅

**What this story does NOT do:**
- ❌ Queue Worker / job processing loop (Story 3.5)
- ❌ Auto-retry logic (Story 3.6)
- ❌ `PushToQueueHandler` event handler (chú ý AC-3 bên dưới — epics yêu cầu handler nhưng EventBus hiện là no-op InMemoryEventBus, nên implement handler stub)
- ❌ Thay đổi DB schema

**Depends on:** Stories 3.6 ✅ + 3.7 ✅ + 3.3 (ready-for-dev, không cần done)

## Acceptance Criteria

### AC-1: QueueManager Trait

**Given** cần abstraction cho queue
**When** I create `src-tauri/src/infrastructure/queue/queue_manager.rs`
**Then** phải định nghĩa:

```rust
use crate::domain::print_job::aggregate::PrintJob;
use crate::domain::print_job::errors::DomainError;
use crate::domain::print_job::value_objects::JobId;

/// Errors specific to queue operations.
#[derive(Debug)]
pub enum QueueError {
    /// Underlying repository failure.
    RepositoryError(String),
    /// Job not found when expected.
    JobNotFound(String),
    /// Job is in wrong state for this operation.
    InvalidState(String),
}

impl std::fmt::Display for QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RepositoryError(msg) => write!(f, "Queue repository error: {}", msg),
            Self::JobNotFound(id) => write!(f, "Job not found: {}", id),
            Self::InvalidState(msg) => write!(f, "Invalid job state: {}", msg),
        }
    }
}

impl std::error::Error for QueueError {}

/// Contract for the durable print job queue.
pub trait QueueManager: Send + Sync {
    /// Move a PENDING job to QUEUED status.
    /// Returns QueueError::JobNotFound if job doesn't exist.
    /// Returns QueueError::InvalidState if job is not PENDING.
    fn push(&self, job_id: &JobId) -> Result<(), QueueError>;

    /// Pop the oldest QUEUED job and transition it to PENDING (for worker pickup).
    /// Returns None if queue is empty.
    /// Uses SELECT with ORDER BY created_at ASC LIMIT 1.
    fn pop(&self) -> Result<Option<PrintJob>, QueueError>;

    /// Return a job to QUEUED state (e.g., after transient failure).
    /// `delay_secs`: future time offset (for exponential backoff in Story 3.6).
    /// For this story: ignore delay_secs, just set status back to QUEUED.
    fn requeue(&self, job_id: &JobId, delay_secs: u64) -> Result<(), QueueError>;

    /// Return current count of QUEUED jobs.
    fn queue_depth(&self) -> Result<usize, QueueError>;
}
```

### AC-2: SqliteQueueManager Implementation

**Given** QueueManager trait defined
**When** I implement `SqliteQueueManager`
**Then** `src-tauri/src/infrastructure/queue/sqlite_queue_manager.rs` phải:

**Struct:**
```rust
pub struct SqliteQueueManager {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteQueueManager {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }
}
```

**`push()` implementation:**
```rust
// 1. Load job by ID (SELECT status FROM print_jobs WHERE id = ?)
// 2. Verify status == "Pending" → else QueueError::InvalidState
// 3. UPDATE print_jobs SET status = 'Queued', updated_at = now WHERE id = ?
// Returns QueueError::JobNotFound if 0 rows affected on SELECT
```

**`pop()` implementation:**
```rust
// SELECT id, printer_name, document_url, status, retry_count
// FROM print_jobs
// WHERE status = 'Queued'
// ORDER BY created_at ASC
// LIMIT 1
//
// If found: UPDATE status = 'Pending', updated_at = now (mark as "being processed")
// Reconstruct PrintJob via PrintJob::reconstruct()
// Returns Ok(None) if no QUEUED rows
```

> **Lưu ý pop():** Pop chuyển trạng thái về Pending (không phải trạng thái "Processing" mới) để tận dụng PrintStatus enum hiện có. Story 3.5 (Worker) sẽ gọi `job.queue()` ngay sau khi pop để xử lý.

**`requeue()` implementation:**
```rust
// UPDATE print_jobs SET status = 'Queued', updated_at = now WHERE id = ?
// delay_secs: log nhưng không dùng (Story 3.6 sẽ xử lý)
```

**`queue_depth()` implementation:**
```rust
// SELECT COUNT(*) FROM print_jobs WHERE status = 'Queued'
```

**SQL column mapping:**
- `id` → `job.id().to_string()`
- `document_url` → `job.pdf_url()` (column name trong DB là `document_url`, không phải `pdf_url`)
- `printer_name` → `job.printer_name()`
- `status` → dùng `{:?}` format (vd: "Queued", "Pending")

### AC-3: PushToQueueHandler (Event Handler Stub)

**Given** epics yêu cầu handler lắng nghe PrintJobCreated và gọi queue_manager.push
**When** I create `src-tauri/src/application/handlers/push_to_queue_handler.rs`
**Then** phải có:

```rust
use std::sync::Arc;
use crate::domain::print_job::value_objects::JobId;
use crate::infrastructure::queue::QueueManager;

/// Handles PrintJobCreated event by pushing job to the durable queue.
///
/// Called AFTER CreatePrintJobUseCase saves the job.
/// In MVP: called directly from use case (not via async event bus).
pub struct PushToQueueHandler {
    pub queue_manager: Arc<dyn QueueManager>,
}

impl PushToQueueHandler {
    pub fn new(queue_manager: Arc<dyn QueueManager>) -> Self {
        Self { queue_manager }
    }

    /// Push a newly created job to the queue.
    pub fn handle(&self, job_id: &JobId) -> Result<(), String> {
        self.queue_manager
            .push(job_id)
            .map_err(|e| format!("Failed to push job to queue: {}", e))
    }
}
```

> **Tại sao stub:** `InMemoryEventBus` là no-op — events không được dispatch tự động. Handler này được gọi trực tiếp từ `CreatePrintJobUseCase` trong Story 3.3 (hoặc wired thủ công). Story này chỉ tạo struct/impl, không wire vào EventBus.

### AC-4: Module Exports

**Given** code tạo xong
**When** I update mod files
**Then:**

`src-tauri/src/infrastructure/queue/mod.rs`:
```rust
pub mod queue_manager;
pub mod sqlite_queue_manager;

pub use queue_manager::{QueueError, QueueManager};
pub use sqlite_queue_manager::SqliteQueueManager;
```

`src-tauri/src/application/handlers/mod.rs`:
```rust
pub mod push_to_queue_handler;
pub use push_to_queue_handler::PushToQueueHandler;
```

### AC-5: AppContextState Extension

**Given** QueueManager cần được available ở Tauri commands
**When** I update `main.rs`
**Then:**

```rust
// Thêm vào AppContextState struct (dòng 278-283):
struct AppContextState {
    printer_repo: Arc<dyn sapo_printer::domain::printer::PrinterRepository>,
    printer_manager: Arc<dyn PrinterManager>,
    _secret_manager: Arc<dyn SecretManager>,
    job_repo: Arc<dyn sapo_printer::domain::print_job::PrintJobRepository>,   // từ Story 3.3
    event_store: Arc<sapo_printer::infrastructure::database::SqliteEventStore>, // từ Story 3.3
    event_bus: Arc<dyn sapo_printer::shared::event_bus::EventBus>,             // từ Story 3.3
    queue_manager: Arc<dyn sapo_printer::infrastructure::queue::QueueManager>, // NEW
}
```

```rust
// Wire trong main() sau job_repo/event_store:
use sapo_printer::infrastructure::queue::SqliteQueueManager;

let queue_manager: Arc<dyn sapo_printer::infrastructure::queue::QueueManager> =
    Arc::new(SqliteQueueManager::new(pool.get_arc()));
```

> **Note:** Nếu Story 3.3 chưa done, vẫn phải thêm `job_repo`, `event_store`, `event_bus` vào `AppContextState` — đây là prerequisite shared.

### AC-6: Unit Tests

**File:** inline `#[cfg(test)]` trong `sqlite_queue_manager.rs`

**Setup helper:**
```rust
fn setup() -> (Arc<Mutex<Connection>>, SqliteQueueManager) {
    let mut conn = Connection::open_in_memory().unwrap();
    run_migrations(&mut conn).unwrap();
    let arc = Arc::new(Mutex::new(conn));
    let mgr = SqliteQueueManager::new(arc.clone());
    (arc, mgr)
}

fn insert_job(conn: &Arc<Mutex<Connection>>, job_id: &str, status: &str) {
    let c = conn.lock().unwrap();
    let now = 1_700_000_000i64;
    c.execute(
        "INSERT INTO print_jobs (id, printer_name, document_url, status, retry_count, created_at, updated_at)
         VALUES (?1, 'HP', 'https://s3.example.com/doc.pdf', ?2, 0, ?3, ?3)",
        rusqlite::params![job_id, status, now],
    ).unwrap();
}
```

**Required tests:**

1. **`test_push_pending_to_queued`** — PENDING job → push() → status becomes "Queued"
2. **`test_push_non_pending_returns_invalid_state`** — already QUEUED job → push() → `QueueError::InvalidState`
3. **`test_push_nonexistent_job_returns_not_found`** — random UUID → push() → `QueueError::JobNotFound`
4. **`test_pop_returns_oldest_queued`** — 2 QUEUED jobs (different created_at) → pop() → returns older one
5. **`test_pop_empty_queue_returns_none`** — no QUEUED jobs → pop() → `Ok(None)`
6. **`test_pop_sets_status_to_pending`** — after pop(), job status in DB is "Pending"
7. **`test_requeue_sets_status_to_queued`** — PENDING job → requeue() → status becomes "Queued"
8. **`test_queue_depth_counts_queued`** — 3 QUEUED + 2 PENDING → queue_depth() → 3
9. **`test_queue_depth_empty`** — no jobs → queue_depth() → 0
10. **`test_fifo_ordering`** — push 3 jobs with sequential created_at → pop 3 times → FIFO order

### AC-7: Integration Test

**File:** `src-tauri/tests/integration/queue_manager_integration_test.rs`

```rust
// Test: jobs persist "across restart" (simulate with new manager on same DB)
#[test]
fn test_jobs_persist_across_simulated_restart() {
    let mut conn = Connection::open_in_memory().unwrap();
    run_migrations(&mut conn).unwrap();
    let arc_conn = Arc::new(Mutex::new(conn));

    // "Session 1": insert PENDING job, push to queue
    let job_repo = SqlitePrintJobRepository::new(arc_conn.clone());
    let mgr1 = SqliteQueueManager::new(arc_conn.clone());
    let job = PrintJob::new("https://s3.example.com/doc.pdf".into(), "HP".into());
    let job_id = job.id().clone();
    job_repo.save(&job).unwrap();
    mgr1.push(&job_id).unwrap();

    // "Session 2": new QueueManager instance, same DB — job must still be there
    let mgr2 = SqliteQueueManager::new(arc_conn.clone());
    let depth = mgr2.queue_depth().unwrap();
    assert_eq!(depth, 1);

    let popped = mgr2.pop().unwrap();
    assert!(popped.is_some());
    assert_eq!(popped.unwrap().id(), &job_id);
}
```

Register trong `tests/integration/mod.rs`.

### AC-8: All Tests Pass

- `cargo test` — all pass
- `cargo check` — zero errors
- `cargo clippy -- -D warnings` — clean
- `cargo fmt --check` — passes

## Tasks / Subtasks

- [ ] Task 1: QueueManager trait + QueueError (AC-1)
  - [ ] Tạo `src-tauri/src/infrastructure/queue/queue_manager.rs`

- [ ] Task 2: SqliteQueueManager (AC-2)
  - [ ] Tạo `src-tauri/src/infrastructure/queue/sqlite_queue_manager.rs`
  - [ ] Implement push, pop, requeue, queue_depth

- [ ] Task 3: PushToQueueHandler stub (AC-3)
  - [ ] Tạo `src-tauri/src/application/handlers/push_to_queue_handler.rs`

- [ ] Task 4: Module exports (AC-4)
  - [ ] Update `infrastructure/queue/mod.rs`
  - [ ] Update `application/handlers/mod.rs`

- [ ] Task 5: AppContextState + main.rs (AC-5)
  - [ ] Extend `AppContextState` struct
  - [ ] Wire `SqliteQueueManager::new(pool.get_arc())` trong `main()`

- [ ] Task 6: Unit tests (AC-6)
  - [ ] 10 unit tests trong `sqlite_queue_manager.rs`

- [ ] Task 7: Integration test (AC-7)
  - [ ] Tạo `tests/integration/queue_manager_integration_test.rs`
  - [ ] Register trong `tests/integration/mod.rs`

- [ ] Task 8: Final verification (AC-8)
  - [ ] `cargo test` | `cargo check` | `cargo clippy` | `cargo fmt`

## Dev Notes

### File Structure

```
src-tauri/src/infrastructure/queue/
├── mod.rs                        # UPDATE: pub mod queue_manager; pub mod sqlite_queue_manager;
├── queue_manager.rs              # NEW: QueueManager trait + QueueError
└── sqlite_queue_manager.rs       # NEW: SqliteQueueManager + unit tests

src-tauri/src/application/handlers/
├── mod.rs                        # UPDATE: pub mod push_to_queue_handler;
└── push_to_queue_handler.rs      # NEW: PushToQueueHandler

src-tauri/src/main.rs             # UPDATE: AppContextState + wire queue_manager
src-tauri/tests/integration/
├── mod.rs                        # UPDATE: mod queue_manager_integration_test;
└── queue_manager_integration_test.rs  # NEW
```

### Database Column Mapping — CRITICAL

`print_jobs` table columns (từ `migrations.rs` MIGRATION_3):
- `id TEXT PRIMARY KEY CHECK(length(id) = 36)` — UUID string
- `printer_name TEXT`
- `document_url TEXT` ← **tên column là `document_url`**, không phải `pdf_url`
- `status TEXT DEFAULT 'PENDING'` ← **default là uppercase "PENDING"** nhưng code dùng `{:?}` format → "Pending"
- `retry_count INTEGER DEFAULT 0`
- `created_at INTEGER NOT NULL`
- `updated_at INTEGER NOT NULL`
- `completed_at INTEGER` (nullable)

> **WARNING:** `status_to_string()` trong `print_job_repository.rs` dùng `format!("{:?}", s)` → "Pending", "Queued" (CamelCase, không phải UPPER_CASE). SQL queries phải dùng đúng case này.

### pop() — Lý do dùng Pending thay vì trạng thái mới

Khi `pop()` được gọi, job chuyển từ "Queued" → "Pending" (tạm thời, trong khi worker xử lý). Worker (Story 3.5) sẽ ngay lập tức gọi `job.queue()` để chuyển về Queued, rồi xử lý pipeline. Đây là design đơn giản nhất tránh thêm trạng thái "Processing" mới vào PrintStatus enum.

**Thứ tự trong pop():**
1. SELECT ... WHERE status = 'Queued' ORDER BY created_at ASC LIMIT 1
2. Nếu found: UPDATE status = 'Pending', updated_at = now
3. Reconstruct PrintJob từ row data
4. Return Ok(Some(job))

### push() — Load job cách nào?

`push()` nhận `&JobId` không phải `&PrintJob`. Cần check status trong DB:

```rust
pub fn push(&self, job_id: &JobId) -> Result<(), QueueError> {
    let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
    let id_str = job_id.to_string();

    // Check current status
    let result: rusqlite::Result<String> = conn.query_row(
        "SELECT status FROM print_jobs WHERE id = ?1",
        [&id_str],
        |row| row.get(0),
    );

    match result {
        Err(rusqlite::Error::QueryReturnedNoRows) =>
            return Err(QueueError::JobNotFound(id_str)),
        Err(e) =>
            return Err(QueueError::RepositoryError(e.to_string())),
        Ok(status) if status != "Pending" =>
            return Err(QueueError::InvalidState(
                format!("Expected Pending, got {}", status)
            )),
        Ok(_) => {} // Pending — proceed
    }

    // Update to Queued
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    conn.execute(
        "UPDATE print_jobs SET status = 'Queued', updated_at = ?1 WHERE id = ?2",
        rusqlite::params![now, id_str],
    ).map_err(|e| QueueError::RepositoryError(e.to_string()))?;

    Ok(())
}
```

### Imports cần thiết cho sqlite_queue_manager.rs

```rust
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use crate::domain::print_job::aggregate::PrintJob;
use crate::domain::print_job::value_objects::{JobId, PrintStatus};
use crate::infrastructure::queue::queue_manager::{QueueError, QueueManager};
```

### row_to_print_job helper — tái sử dụng

`print_job_repository.rs` có `row_to_print_job()` nhưng là `fn` private. `SqliteQueueManager` cần row mapping tương tự. Có 2 options:
1. **Copy pattern:** implement inline trong `sqlite_queue_manager.rs` (đơn giản nhất)
2. **Expose:** move `row_to_print_job` lên `pub(crate)` trong repository

→ **Khuyến nghị option 1** để không thay đổi repository đã stable.

**Row mapping và Transaction cho pop():**
```rust
// Trong hàm pop() của SqliteQueueManager:
pub fn pop(&self) -> Result<Option<PrintJob>, QueueError> {
    let mut conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());

    // Dùng transaction để tránh race condition giữa các worker
    let tx = conn.transaction().map_err(|e| QueueError::RepositoryError(e.to_string()))?;

    let job_opt = {
        let mut stmt = tx.prepare(
            "SELECT id, printer_name, document_url, retry_count
             FROM print_jobs WHERE status = 'Queued'
             ORDER BY created_at ASC LIMIT 1"
        ).map_err(|e| QueueError::RepositoryError(e.to_string()))?;

        let result = stmt.query_row([], |row| {
            let id_str: String = row.get(0)?;
            let printer_name: String = row.get(1)?;
            let document_url: String = row.get(2)?;
            let retry_count: i64 = row.get(3)?;
            
            let id: JobId = id_str.parse().map_err(|e: uuid::Error| {
                rusqlite::Error::InvalidColumnType(0, e.to_string(), rusqlite::types::Type::Text)
            })?;
            
            // Reconstruct với status = Pending (vì ta sắp update nó thành Pending)
            Ok(PrintJob::reconstruct(
                id,
                PrintStatus::Pending,
                retry_count as u32,
                document_url,
                printer_name,
            ))
        });

        match result {
            Ok(job) => Some(job),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(QueueError::RepositoryError(e.to_string())),
        }
    };

    if let Some(job) = &job_opt {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
        tx.execute(
            "UPDATE print_jobs SET status = 'Pending', updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, job.id().to_string()],
        ).map_err(|e| QueueError::RepositoryError(e.to_string()))?;
    }

    tx.commit().map_err(|e| QueueError::RepositoryError(e.to_string()))?;

    Ok(job_opt)
}
```

### main.rs — AppContextState hiện tại (dòng 278-283)

```rust
// Hiện tại:
struct AppContextState {
    printer_repo: Arc<dyn sapo_printer::domain::printer::PrinterRepository>,
    printer_manager: Arc<dyn PrinterManager>,
    _secret_manager: Arc<dyn SecretManager>,
}
```

Phải thêm `job_repo`, `event_store`, `event_bus` (Story 3.3) VÀ `queue_manager` (story này). Nếu Story 3.3 chưa merge, thêm tất cả cùng lúc.

### integration/mod.rs — Kiểm tra trước khi update

Trước khi thêm `mod queue_manager_integration_test;`, kiểm tra `tests/integration/mod.rs` có sẵn chưa (từ Story 3.7). Pattern: thêm `mod queue_manager_integration_test;` vào file.

### Environment Note

`cargo build` có thể bị WDAC block (`os error 4551`). Dùng `cargo check` để verify compilation.

## Previous Story Learnings

### From Story 3.7 (SqlitePrintJobRepository):
- `setup_test_db()`: `Connection::open_in_memory()` + `run_migrations(&mut conn)` + `Arc::new(Mutex::new(conn))`
- `unwrap_or_else(|p| p.into_inner())` cho mutex poison recovery
- `rusqlite::Error::QueryReturnedNoRows` pattern cho "not found"
- Column `document_url` lưu PDF URL (không phải `pdf_url`)
- `status_to_string()` dùng `{:?}` → "Queued", "Pending" (CamelCase)

### From Story 3.3 (CreatePrintJobUseCase — ready-for-dev):
- `AppContextState` cần extend thêm `job_repo`, `event_store`, `event_bus`
- Commands được define trong `main.rs` (không phải `lib.rs`)
- Handler registration: `.invoke_handler(tauri::generate_handler![...])` trong `main.rs` dòng 265

### From Story 3.6 (Schema — done):
- `print_jobs` indexes: `idx_print_jobs_status` + `idx_print_jobs_created_at` ✅
- `ORDER BY created_at ASC` sẽ dùng index → performant

## References

- `epics.md` — Story 3.4 ACs (lines 937–952)
- `src-tauri/src/infrastructure/database/migrations.rs` — print_jobs schema (MIGRATION_3)
- `src-tauri/src/infrastructure/database/print_job_repository.rs` — SQL patterns + row mapping
- `src-tauri/src/domain/print_job/aggregate.rs` — PrintJob::reconstruct()
- `src-tauri/src/domain/print_job/value_objects.rs` — PrintStatus variants
- `src-tauri/src/infrastructure/queue/mod.rs` — stub hiện tại
- `src-tauri/src/application/handlers/mod.rs` — handlers module
- `src-tauri/src/main.rs` — AppContextState (dòng 278-283), invoke_handler (dòng 265)

## Dev Agent Record

### Agent Model Used
(to be filled)

### Completion Notes List
(to be filled)

### File List
(to be filled)
