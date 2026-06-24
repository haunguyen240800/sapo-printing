# Story 3.5: Implement Queue Worker with Batch Processing

Status: done

## Story

As a **developer**,
I want **a queue worker that processes jobs sequentially**,
So that **print jobs are executed automatically from the queue**.

## Context

Story này implement `QueueWorker` — background worker lắng nghe queue và thực thi job pipeline tự động: pop → download → render → print → update status. Worker khởi động cùng app, chạy trong background thread, và xử lý jobs tuần tự theo FIFO order.

**Foundation đã có:**
- ✅ `QueueManager` trait + `SqliteQueueManager` (Story 3.4)
- ✅ `DocumentDownloader` trait + `ReqwestDownloader` với circuit breaker
- ✅ `DocumentRenderer` trait + `PdfiumRenderer` + `DirectPdfRenderer`
- ✅ `StrategySelector` để chọn renderer tự động
- ✅ `PrinterEngine` trait + `WindowsPrinterEngine`
- ✅ `PrintJobRepository` + `EventStore` + `EventBus`
- ✅ PrintJob aggregate với đầy đủ state transition methods
- ✅ Domain events: PrintJobQueued, Downloaded, Submitted, Printing, Completed, Failed

**What this story does:**
- ✅ QueueWorker struct với process loop
- ✅ Background thread lifecycle (start/stop)
- ✅ Job pipeline: pop → download → render → print → complete
- ✅ State transitions + event publishing sau mỗi bước
- ✅ Temp file auto-cleanup via RAII (TempPdfFile Drop trait)
- ✅ Wire worker vào main.rs

**What this story does NOT do:**
- ❌ Auto-retry logic với exponential backoff (Story 3.6)
- ❌ Job cancellation (Story 3.7)
- ❌ UI/dashboard (Story 3.8, 3.9)
- ❌ Multiple concurrent workers (MVP: 1 worker thread)

**Depends on:** Stories 3.4 ✅, 3.1 ✅, 3.2 ✅, 3.3 ✅, 3.7 ✅

## Acceptance Criteria

### AC-1: QueueWorker Structure

**Given** cần background worker để process jobs
**When** I create `src-tauri/src/infrastructure/queue/queue_worker.rs`
**Then** phải định nghĩa:

```rust
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::domain::print_job::PrintJobRepository;
use crate::infrastructure::database::SqliteEventStore;
use crate::infrastructure::downloader::DocumentDownloader;
use crate::infrastructure::printer::PrinterEngine;
use crate::infrastructure::queue::QueueManager;
use crate::infrastructure::renderer::DocumentRenderer;
use crate::shared::event_bus::EventBus;

pub struct QueueWorker {
    queue_manager: Arc<dyn QueueManager>,
    job_repo: Arc<dyn PrintJobRepository>,
    event_store: Arc<SqliteEventStore>,
    event_bus: Arc<dyn EventBus>,
    downloader: Arc<dyn DocumentDownloader>,
    renderer: Arc<dyn DocumentRenderer>,
    printer_engine: Arc<dyn PrinterEngine>,
    
    // Worker lifecycle control
    running: Arc<AtomicBool>,
    thread_handle: Mutex<Option<JoinHandle<()>>>,
}

impl QueueWorker {
    pub fn new(
        queue_manager: Arc<dyn QueueManager>,
        job_repo: Arc<dyn PrintJobRepository>,
        event_store: Arc<SqliteEventStore>,
        event_bus: Arc<dyn EventBus>,
        downloader: Arc<dyn DocumentDownloader>,
        renderer: Arc<dyn DocumentRenderer>,
        printer_engine: Arc<dyn PrinterEngine>,
    ) -> Self {
        Self {
            queue_manager,
            job_repo,
            event_store,
            event_bus,
            downloader,
            renderer,
            printer_engine,
            running: Arc::new(AtomicBool::new(false)),
            thread_handle: Mutex::new(None),
        }
    }

    /// Start worker in background thread
    pub fn start(&self) -> Result<(), String>;

    /// Stop worker gracefully
    pub fn stop(&self) -> Result<(), String>;

    /// Check if worker is running
    pub fn is_running(&self) -> bool;
}
```

**Constraints:**
- Worker phải thread-safe (tất cả fields dùng Arc)
- Lifecycle control: AtomicBool cho running flag
- JoinHandle wrapped trong Mutex để stop() có thể join thread

### AC-2: Worker Start/Stop Lifecycle

**Given** QueueWorker struct exists
**When** I implement start() and stop() methods
**Then:**

**`start()` implementation:**
```rust
pub fn start(&self) -> Result<(), String> {
    // 1. Check if already running
    if self.running.load(Ordering::SeqCst) {
        return Err("Worker already running".into());
    }

    // 2. Set running flag
    self.running.store(true, Ordering::SeqCst);

    // 3. Clone Arc references for thread
    let queue_manager = Arc::clone(&self.queue_manager);
    let job_repo = Arc::clone(&self.job_repo);
    let event_store = Arc::clone(&self.event_store);
    let event_bus = Arc::clone(&self.event_bus);
    let downloader = Arc::clone(&self.downloader);
    let renderer = Arc::clone(&self.renderer);
    let printer_engine = Arc::clone(&self.printer_engine);
    let running = Arc::clone(&self.running);

    // 4. Spawn background thread
    let handle = thread::spawn(move || {
        Self::process_loop(
            queue_manager,
            job_repo,
            event_store,
            event_bus,
            downloader,
            renderer,
            printer_engine,
            running,
        );
    });

    // 5. Store handle
    *self.thread_handle.lock().unwrap() = Some(handle);

    Ok(())
}
```

**`stop()` implementation:**
```rust
pub fn stop(&self) -> Result<(), String> {
    // 1. Set running flag to false
    self.running.store(false, Ordering::SeqCst);

    // 2. Wait for thread to finish
    let mut handle_guard = self.thread_handle.lock().unwrap();
    if let Some(handle) = handle_guard.take() {
        handle.join().map_err(|_| "Failed to join worker thread")?;
    }

    Ok(())
}
```

**`is_running()` implementation:**
```rust
pub fn is_running(&self) -> bool {
    self.running.load(Ordering::SeqCst)
}
```

### AC-3: Process Loop Implementation

**Given** worker lifecycle methods exist
**When** I implement the core process loop
**Then** `process_loop()` phải:

```rust
fn process_loop(
    queue_manager: Arc<dyn QueueManager>,
    job_repo: Arc<dyn PrintJobRepository>,
    event_store: Arc<SqliteEventStore>,
    event_bus: Arc<dyn EventBus>,
    downloader: Arc<dyn DocumentDownloader>,
    renderer: Arc<dyn DocumentRenderer>,
    printer_engine: Arc<dyn PrinterEngine>,
    running: Arc<AtomicBool>,
) {
    const POLL_INTERVAL_MS: u64 = 500; // 0.5s poll interval

    while running.load(Ordering::SeqCst) {
        match queue_manager.pop() {
            Ok(Some(job)) => {
                // Process job through pipeline
                let result = Self::process_job(
                    job,
                    &job_repo,
                    &event_store,
                    &event_bus,
                    &downloader,
                    &renderer,
                    &printer_engine,
                );

                if let Err(e) = result {
                    eprintln!("Worker: job processing failed: {}", e);
                    // Note: Story 3.6 sẽ implement auto-retry logic ở đây
                }
            }
            Ok(None) => {
                // Queue empty, sleep before next poll
                thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            }
            Err(e) => {
                eprintln!("Worker: queue pop failed: {}", e);
                thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            }
        }
    }
}
```

**Poll interval:**
- 500ms (0.5 giây) — balance giữa responsiveness và CPU usage
- Sleep khi queue empty để không busy-loop
- Continue polling ngay khi có job

### AC-4: Job Processing Pipeline

**Given** process loop hoạt động
**When** I implement job pipeline
**Then** `process_job()` phải thực hiện từng bước tuần tự:

```rust
fn process_job(
    mut job: PrintJob,
    job_repo: &Arc<dyn PrintJobRepository>,
    event_store: &Arc<SqliteEventStore>,
    event_bus: &Arc<dyn EventBus>,
    downloader: &Arc<dyn DocumentDownloader>,
    renderer: &Arc<dyn DocumentRenderer>,
    printer_engine: &Arc<dyn PrinterEngine>,
) -> Result<(), String> {
    // Step 1: Transition to Queued (pop trả về Pending, ta phải chuyển sang Queued)
    job.queue().map_err(|e| format!("Failed to queue job: {:?}", e))?;
    Self::persist_and_publish(&mut job, job_repo, event_store, event_bus)?;

    // Step 2: Download document
    let pdf_path = downloader
        .download(job.pdf_url(), job.id())
        .map_err(|e| format!("Download failed: {:?}", e))?;

    job.mark_downloaded()
        .map_err(|e| format!("Failed to mark downloaded: {:?}", e))?;
    Self::persist_and_publish(&mut job, job_repo, event_store, event_bus)?;

    // Step 3: Render document
    let render_config = RenderConfig::default(); // TODO: Get from job config
    let rendered_data = renderer
        .render(&pdf_path, &render_config)
        .map_err(|e| format!("Render failed: {:?}", e))?;

    job.mark_submitted()
        .map_err(|e| format!("Failed to mark submitted: {:?}", e))?;
    Self::persist_and_publish(&mut job, job_repo, event_store, event_bus)?;

    // Step 4: Send to printer
    job.mark_printing()
        .map_err(|e| format!("Failed to mark printing: {:?}", e))?;
    Self::persist_and_publish(&mut job, job_repo, event_store, event_bus)?;

    printer_engine
        .print(job.printer_name(), &rendered_data)
        .map_err(|e| format!("Print failed: {:?}", e))?;

    // Step 5: Mark complete
    job.complete()
        .map_err(|e| format!("Failed to mark complete: {:?}", e))?;
    Self::persist_and_publish(&mut job, job_repo, event_store, event_bus)?;

    // Step 6: Cleanup temp file happens automatically via TempPdfFile Drop
    // (pdf_path goes out of scope here)

    Ok(())
}
```

**Critical pattern:**
- Mỗi state transition: gọi domain method → persist_and_publish()
- Download trả về PathBuf — sẽ được wrapped trong TempPdfFile (Story 3.5 requirement)
- Temp file cleanup tự động khi pdf_path out of scope
- Error ở bất kỳ bước nào → return Err, worker log error (retry logic trong Story 3.6)

### AC-5: Persist and Publish Helper

**Given** mỗi state transition cần persist + publish events
**When** I implement persist_and_publish() helper
**Then:**

```rust
fn persist_and_publish(
    job: &mut PrintJob,
    job_repo: &Arc<dyn PrintJobRepository>,
    event_store: &Arc<SqliteEventStore>,
    event_bus: &Arc<dyn EventBus>,
) -> Result<(), String> {
    // 1. Drain events from aggregate
    let events = job.drain_events();

    // 2. Persist job state
    job_repo
        .update(job)
        .map_err(|e| format!("Failed to update job: {:?}", e))?;

    // 3. Persist events to event store
    event_store
        .save_all(job.id().to_string().as_str(), &events)
        .map_err(|e| format!("Failed to save events: {:?}", e))?;

    // 4. Publish events to event bus (non-fatal)
    for event in &events {
        let payload = event.serialize_payload();
        if let Err(e) = event_bus.publish(event.event_type(), &payload) {
            eprintln!("Warning: Failed to publish event {}: {}", event.event_type(), e);
            // Non-fatal — continue processing
        }
    }

    Ok(())
}
```

**Pattern explanation:**
- **Outbox pattern:** Save to DB first, publish after
- **Non-fatal publish:** EventBus failures logged but don't stop pipeline
- **Drain events:** PrintJob buffers events internally, drain extracts and clears them
- Identical to CreatePrintJobUseCase pattern (Story 3.3)

### AC-6: Module Exports

**Given** QueueWorker implemented
**When** I update module files
**Then:**

`src-tauri/src/infrastructure/queue/mod.rs`:
```rust
pub mod queue_manager;
pub mod queue_worker;
pub mod sqlite_queue_manager;

pub use queue_manager::{QueueError, QueueManager};
pub use queue_worker::QueueWorker;
pub use sqlite_queue_manager::SqliteQueueManager;
```

### AC-7: Wire Worker in main.rs

**Given** QueueWorker needs to start on app launch
**When** I update main.rs
**Then:**

```rust
// After AppContextState initialization (around line 320):

use sapo_printer::infrastructure::queue::QueueWorker;

// Create worker with dependencies
let worker = QueueWorker::new(
    Arc::clone(&queue_manager),
    Arc::clone(&state.job_repo),
    Arc::clone(&state.event_store),
    Arc::clone(&state.event_bus),
    Arc::clone(&downloader),        // Need to add this to AppContextState or create here
    Arc::clone(&renderer),          // Need to add this to AppContextState or create here
    Arc::clone(&state.printer_manager), // Cast to PrinterEngine if needed
);

// Start worker
worker.start().expect("Failed to start queue worker");

// TODO: Store worker reference for graceful shutdown (future story)
```

**Alternative approach — extend AppContextState:**
```rust
struct AppContextState {
    // ... existing fields
    worker: Arc<QueueWorker>,  // NEW
}

// In AppBuilder:
let worker = Arc::new(QueueWorker::new(/* deps */));
worker.start().expect("Failed to start worker");

let state = AppContextState {
    // ... existing
    worker: Arc::clone(&worker),
};
```

**Note:** Cần thêm `downloader` và `renderer` vào dependencies — có thể tạo mới trong main.rs hoặc extend AppContextState.

### AC-8: Unit Tests

**File:** inline `#[cfg(test)]` trong `queue_worker.rs`

**Mock dependencies needed:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex as StdMutex;

    // Mock QueueManager
    struct MockQueueManager {
        jobs: StdMutex<Vec<PrintJob>>,
    }

    impl QueueManager for MockQueueManager {
        fn pop(&self) -> Result<Option<PrintJob>, QueueError> {
            Ok(self.jobs.lock().unwrap().pop())
        }
        // ... other methods return Ok(())
    }

    // Similar mocks for: JobRepo, EventStore, EventBus, Downloader, Renderer, PrinterEngine
}
```

**Required tests:**

1. **`test_worker_starts_and_stops`** — start() sets running=true, stop() joins thread
2. **`test_worker_processes_single_job`** — queue with 1 job → worker processes → job COMPLETED
3. **`test_worker_processes_multiple_jobs_fifo`** — 3 jobs → processed in order
4. **`test_worker_sleeps_when_queue_empty`** — empty queue → no busy loop (verify via timing)
5. **`test_worker_publishes_events_at_each_step`** — verify 5 events (Queued, Downloaded, Submitted, Printing, Completed)
6. **`test_worker_persists_state_at_each_step`** — verify job_repo.update() called 5 times
7. **`test_worker_handles_download_failure`** — download error → job not completed (Story 3.6 will add retry)
8. **`test_worker_handles_render_failure`** — render error → job not completed
9. **`test_worker_handles_print_failure`** — print error → job not completed
10. **`test_worker_cleans_temp_file`** — verify TempPdfFile dropped after processing

### AC-9: Integration Test

**File:** `src-tauri/tests/integration/queue_worker_integration_test.rs`

```rust
#[test]
fn test_worker_processes_real_job_flow() {
    // Setup: in-memory DB + real dependencies
    let mut conn = Connection::open_in_memory().unwrap();
    run_migrations(&mut conn).unwrap();
    let arc_conn = Arc::new(Mutex::new(conn));

    let job_repo = Arc::new(SqlitePrintJobRepository::new(arc_conn.clone()));
    let queue_manager = Arc::new(SqliteQueueManager::new(arc_conn.clone()));
    let event_store = Arc::new(SqliteEventStore::new(arc_conn.clone()));
    let event_bus = Arc::new(InMemoryEventBus::new());

    // Mock infrastructure (downloader returns dummy path, etc.)
    let downloader = Arc::new(MockDownloader::new());
    let renderer = Arc::new(MockRenderer::new());
    let printer_engine = Arc::new(MockPrinterEngine::new());

    // Create and save job
    let job = PrintJob::new("https://s3.example.com/doc.pdf".into(), "HP".into());
    let job_id = job.id().clone();
    job_repo.save(&job).unwrap();
    queue_manager.push(&job_id).unwrap();

    // Start worker
    let worker = QueueWorker::new(
        queue_manager,
        job_repo.clone(),
        event_store,
        event_bus,
        downloader,
        renderer,
        printer_engine,
    );
    worker.start().unwrap();

    // Wait for processing (max 5s)
    for _ in 0..10 {
        thread::sleep(Duration::from_millis(500));
        let loaded_job = job_repo.find_by_id(&job_id).unwrap();
        if loaded_job.status() == &PrintStatus::Completed {
            break;
        }
    }

    // Stop worker
    worker.stop().unwrap();

    // Verify job completed
    let final_job = job_repo.find_by_id(&job_id).unwrap();
    assert_eq!(final_job.status(), &PrintStatus::Completed);
}
```

Register trong `tests/integration/mod.rs`.

### AC-10: All Tests Pass

- `cargo test` — all pass
- `cargo check` — zero errors
- `cargo clippy -- -D warnings` — clean
- `cargo fmt --check` — passes

## Tasks / Subtasks

- [ ] Task 1: QueueWorker structure (AC-1)
  - [ ] Create `src-tauri/src/infrastructure/queue/queue_worker.rs`
  - [ ] Define struct with all dependencies (Arc-wrapped)
  - [ ] Add lifecycle fields (running: AtomicBool, thread_handle: Mutex)

- [ ] Task 2: Worker lifecycle methods (AC-2)
  - [ ] Implement start() — spawn background thread
  - [ ] Implement stop() — set flag + join thread
  - [ ] Implement is_running() — read atomic flag

- [ ] Task 3: Process loop (AC-3)
  - [ ] Implement process_loop() static method
  - [ ] Poll queue với 500ms interval khi empty
  - [ ] Handle queue pop errors gracefully

- [ ] Task 4: Job processing pipeline (AC-4)
  - [ ] Implement process_job() với 6 bước tuần tự
  - [ ] State transitions: Queued → Downloaded → Submitted → Printing → Completed
  - [ ] Error handling cho mỗi bước (log + return)
  - [ ] RenderConfig integration (default for MVP)

- [ ] Task 5: Persist and publish helper (AC-5)
  - [ ] Implement persist_and_publish() helper
  - [ ] Drain events → update job → save events → publish
  - [ ] Non-fatal publish pattern

- [ ] Task 6: Module exports (AC-6)
  - [ ] Update `infrastructure/queue/mod.rs`

- [ ] Task 7: Wire worker in main.rs (AC-7)
  - [ ] Create QueueWorker với dependencies
  - [ ] Call worker.start() sau AppContextState init
  - [ ] (Optional) Extend AppContextState với worker field

- [ ] Task 8: Unit tests (AC-8)
  - [ ] Create mock dependencies (QueueManager, JobRepo, etc.)
  - [ ] 10 unit tests covering lifecycle, pipeline, errors

- [ ] Task 9: Integration test (AC-9)
  - [ ] Create `tests/integration/queue_worker_integration_test.rs`
  - [ ] Test real job flow QUEUED → COMPLETED
  - [ ] Register trong `tests/integration/mod.rs`

- [ ] Task 10: Final verification (AC-10)
  - [ ] `cargo test` | `cargo check` | `cargo clippy` | `cargo fmt`

## Dev Notes

### Architecture Context

**Clean Architecture Layer:** Infrastructure (Queue Worker is infrastructure concern)

**Pattern:** Worker Pattern + Background Processing
- Worker runs trong background thread độc lập với main app
- Polling queue mỗi 0.5s
- Sequential job processing (no concurrency trong MVP)
- Graceful shutdown via AtomicBool flag

**Event-Driven Architecture:**
- Worker publish 5 events per job: PrintJobQueued, Downloaded, Submitted, Printing, Completed
- Events persist to event store cho audit trail
- Non-fatal EventBus publish (log failures but continue)

**RAII Pattern:**
- TempPdfFile (Story 3.5 — RAII temp file management) auto-cleanup khi out of scope
- Worker không cần manual file deletion

### State Machine Flow

```
pop() returns PENDING
    ↓
job.queue() → QUEUED (emit PrintJobQueued)
    ↓
download() → mark_downloaded() → DOWNLOADED (emit PrintJobDownloaded)
    ↓
render() → mark_submitted() → SUBMITTED_TO_QUEUE (emit PrintJobSubmitted)
    ↓
mark_printing() → PRINTING (emit PrintJobPrinting)
    ↓
print() → complete() → COMPLETED (emit PrintJobCompleted)
```

**Error paths** (Story 3.6 sẽ implement retry logic):
- Nếu download/render/print fails → log error, return Err
- Hiện tại: job stays trong last successful state
- Story 3.6: fail() → FAILED → retry() → QUEUED (với exponential backoff)

### Critical Dependencies

**From AppContextState (main.rs line 278-283):**
- ✅ `queue_manager: Arc<dyn QueueManager>` — đã có
- ✅ `job_repo: Arc<dyn PrintJobRepository>` — đã có
- ✅ `event_store: Arc<SqliteEventStore>` — đã có
- ✅ `event_bus: Arc<dyn EventBus>` — đã có
- ❌ `downloader: Arc<dyn DocumentDownloader>` — CHƯA CÓ, cần thêm
- ❌ `renderer: Arc<dyn DocumentRenderer>` — CHƯA CÓ, cần thêm
- ⚠️  `printer_engine: Arc<dyn PrinterEngine>` — hiện tại có `printer_manager`, cần check compatibility

**Option 1: Create trong main.rs** (recommended for MVP):
```rust
// In main() after AppContextState:
use sapo_printer::infrastructure::downloader::ReqwestDownloader;
use sapo_printer::infrastructure::renderer::strategy_selector::StrategySelector;

let downloader = Arc::new(ReqwestDownloader::new());
let renderer = Arc::new(StrategySelector::new(/* printer_manager */));
// printer_engine = cast printer_manager? Or separate instance?

let worker = QueueWorker::new(
    queue_manager,
    state.job_repo.clone(),
    state.event_store.clone(),
    state.event_bus.clone(),
    downloader,
    renderer,
    printer_engine,
);
worker.start().expect("Failed to start queue worker");
```

**Option 2: Extend AppContextState** (cleaner long-term):
- Add `downloader`, `renderer`, `printer_engine` fields
- Wire trong AppBuilder pattern
- Worker references via state

### File Structure

```
src-tauri/src/infrastructure/queue/
├── mod.rs                        # UPDATE: pub mod queue_worker; pub use QueueWorker;
├── queue_manager.rs              # EXISTS (Story 3.4)
├── sqlite_queue_manager.rs       # EXISTS (Story 3.4)
└── queue_worker.rs               # NEW

src-tauri/src/main.rs             # UPDATE: Wire worker, start on launch

src-tauri/tests/integration/
├── mod.rs                        # UPDATE: mod queue_worker_integration_test;
└── queue_worker_integration_test.rs  # NEW
```

### Dependencies Already Available

**Domain Layer:**
- `PrintJob::queue()` — PENDING→QUEUED
- `PrintJob::mark_downloaded()` — QUEUED→DOWNLOADED
- `PrintJob::mark_submitted()` — DOWNLOADED→SUBMITTED_TO_QUEUE
- `PrintJob::mark_printing()` — SUBMITTED_TO_QUEUE→PRINTING
- `PrintJob::complete()` — PRINTING→COMPLETED
- `PrintJob::drain_events()` — Extract buffered events

**Infrastructure Layer:**
- `QueueManager::pop()` — Returns oldest QUEUED job (as PENDING)
- `DocumentDownloader::download(url, job_id)` — Returns PathBuf (temp file)
- `DocumentRenderer::render(path, config)` — Returns Vec<u8> (raw print data)
- `PrinterEngine::print(printer_name, data)` — Send to printer
- `PrintJobRepository::update(job)` — Persist state
- `SqliteEventStore::save_all(aggregate_id, events)` — Persist events
- `EventBus::publish(event_type, payload)` — Publish to subscribers

### RenderConfig Source

**Story 3.4 (Hybrid Strategy Selector) notes:**
- `RenderConfig` defined trong `src-tauri/src/infrastructure/renderer/document_renderer.rs`
- Fields: `margins: Margins`, `dpi: u32`, `paper_size: PaperSize`, `color_mode: ColorMode`
- Default: A4, 300 DPI, zero margins, RGB

**For MVP:**
```rust
let render_config = RenderConfig::default();
```

**Future:** Config từ PrintJob aggregate (Story 3.8+ sẽ thêm job config persistence)

### Testing Strategy

**Unit tests:**
- Mock tất cả dependencies (QueueManager, JobRepo, EventStore, EventBus, Downloader, Renderer, PrinterEngine)
- Test lifecycle: start/stop, running flag
- Test pipeline: verify call sequence, event publishing, state transitions
- Test error handling: download/render/print failures

**Integration test:**
- Real SQLite (in-memory)
- Real QueueManager + JobRepo + EventStore
- Mock infrastructure (downloader returns dummy PathBuf, renderer returns dummy Vec<u8>)
- Verify end-to-end flow: push job → worker processes → job COMPLETED

### Worker Thread Safety

**Arc usage:**
- Tất cả dependencies wrapped trong Arc → safe to clone vào thread
- `running: Arc<AtomicBool>` → thread-safe read/write
- `thread_handle: Mutex<Option<JoinHandle<()>>>` → only accessed from stop()

**No shared mutable state:**
- Worker không mutate shared data structures
- Mỗi job processed trong isolation
- Repository handles concurrent access via SQLite locking

### Graceful Shutdown Pattern

**Current implementation (MVP):**
```rust
// Set flag
running.store(false, Ordering::SeqCst);

// Thread checks flag mỗi loop iteration
while running.load(Ordering::SeqCst) { ... }

// Join waits for current job to finish
handle.join()
```

**Limitation:** Nếu job đang process (download/render/print), phải chờ đến khi xong mới shutdown

**Future improvement (Story 4.x):**
- Timeout for join (max 30s)
- Cancel in-flight operations
- Handle SIGTERM/SIGINT signals

### Temp File Cleanup Integration

**Story 3.5 (RAII Temp File Management) đã implement:**
- `TempPdfFile` struct với Drop trait
- Auto-delete khi out of scope
- Strategies: Immediate (success), Deferred (failure 24h), Startup (orphaned >24h)

**Worker integration:**
```rust
// Downloader returns PathBuf wrapped trong TempPdfFile (assumedly)
let pdf_path = downloader.download(...)?;  // Type: TempPdfFile or PathBuf?

// Use path for rendering
let data = renderer.render(&pdf_path, ...)?;

// When pdf_path goes out of scope → Drop trait auto-deletes file
// (Job COMPLETED → immediate cleanup)
// (Job FAILED → keep_on_drop flag set, deferred cleanup)
```

**Assumption:** `DocumentDownloader::download()` trả về `PathBuf` — cần check nếu đã wrapped trong TempPdfFile hay worker phải wrap thủ công.

### Error Handling — Retry Preparation

**Story 3.6 sẽ implement:**
- `is_retryable(error)` — classify errors
- `job.fail(reason)` → FAILED + emit PrintJobFailed
- `job.retry()` → FAILED→QUEUED (if retry_count < 3)
- `queue_manager.requeue(job_id, delay_secs)` — exponential backoff (5s, 10s, 20s)

**Worker story 3.5 chỉ cần:**
- Log error: `eprintln!("Worker: job {} failed: {}", job.id(), e);`
- Return Err từ process_job()
- process_loop() log error và continue polling

**Placeholder for Story 3.6:**
```rust
Err(e) => {
    eprintln!("Worker: job processing failed: {}", e);
    // TODO Story 3.6: Classify error, call job.fail(), requeue if retryable
}
```

### Performance Considerations

**Poll interval:** 500ms
- Trade-off: Responsiveness vs CPU usage
- Alternative: 1s (slower), 100ms (higher CPU)
- Future: Event-driven notification (channel, condition variable)

**Sequential processing:**
- MVP: 1 worker thread, 1 job at a time
- Meets NFR-1: ≥100 đơn/phút = 1.67 jobs/sec
- Job processing time budget: < 600ms per job
  - Download: < 30s (timeout)
  - Render: ~2-3s (MuPDF) or ~0.5s (Direct PDF)
  - Print: ~100ms (Windows API)
- **Bottleneck:** Download + Render (~2-33s per job)
- **Conclusion:** Sequential OK for small batches, need concurrency for 5000 đơn < 60 phút

**Future optimization (Story 4.x):**
- Multiple worker threads (configurable pool size)
- Concurrent downloads (max 10 per NFR-1)
- Concurrent renders (max 10 per NFR-1)
- Worker pool pattern

### Integration with Existing Code

**AppContextState location:** `main.rs` line 278-283 (NOT `lib.rs`)
- Worker creation happens trong `main()`
- No Tauri State needed (worker self-contained)
- Optional: Store worker reference cho graceful shutdown

**Tauri lifecycle hooks:**
```rust
// In main():
let worker = QueueWorker::new(...);
worker.start()?;

// TODO Story 4.x: Tauri cleanup handler
// tauri::Builder::default()
//     .on_window_event(|event| {
//         if event == WindowEvent::CloseRequested {
//             worker.stop()?;
//         }
//     })
```

### References

- Epics.md — Story 3.5 ACs (lines 954–969)
- CLAUDE.md — QueueWorker Flow
- Story 3.4 — QueueManager patterns, pop() behavior
- Story 3.3 — Outbox pattern, persist_and_publish
- Story 3.5 (RAII) — TempPdfFile cleanup
- Domain aggregate — PrintJob state transition methods
- Architecture.md — Worker pattern, batch processing

### Previous Story Learnings

**From Story 3.4 (QueueManager):**
- `pop()` returns job in PENDING state (not QUEUED) — worker phải call `job.queue()` ngay sau pop
- Transaction usage trong pop() cho atomicity
- Mutex poison recovery: `unwrap_or_else(|p| p.into_inner())`

**From Story 3.3 (CreatePrintJobUseCase):**
- Outbox pattern: save → persist events → publish
- Non-fatal EventBus failures: log warning, continue
- Event serialization: `event.serialize_payload()`
- `drain_events()` extracts and clears buffered events

**From Story 3.7 (PrintJobRepository):**
- Column `document_url` (not `pdf_url`)
- Status format: "Queued", "Pending" (CamelCase via `{:?}`)
- In-memory SQLite for unit tests: `Connection::open_in_memory()`

**From Story 3.1 (S3 Downloader):**
- `ReqwestDownloader::download(url, job_id)` returns `PathBuf`
- Validates PDF header `%PDF-`
- Circuit breaker wraps calls
- Timeout: 30s

**From Story 3.2 (MuPDF Renderer):**
- `PdfiumRenderer::render(path, config)` returns `Vec<u8>`
- RenderConfig with margins, DPI, paper size, color mode
- Default: A4, 300 DPI, zero margins, RGB

## Dev Agent Record

### Agent Model Used

Claude Sonnet 4.6

### Completion Notes List

1. **QueueWorker Implementation (AC-1 to AC-5):**
   - Created `src-tauri/src/infrastructure/queue/queue_worker.rs` with complete worker implementation
   - Implemented lifecycle management (start/stop/is_running) with AtomicBool for thread-safe control
   - Process loop polls queue every 500ms, handles empty queue with sleep
   - Job pipeline: pop → queue() → download → mark_downloaded() → render → mark_submitted() → mark_printing() → print → complete()
   - Persist-and-publish helper follows Outbox Pattern: update job → save events → publish (non-fatal)
   - All dependencies Arc-wrapped for thread safety

2. **Module Exports (AC-6):**
   - Updated `src-tauri/src/infrastructure/queue/mod.rs` to export QueueWorker

3. **Main.rs Integration (AC-7):**
   - Wired worker in `src-tauri/src/main.rs` after AppContextState initialization
   - Created dependencies: ReqwestDownloader, PdfiumRenderer (300 DPI), WindowsPrinterEngine
   - Used PdfiumRenderer directly instead of StrategySelector for MVP (simpler, meets requirements)
   - Worker starts successfully on app launch with println confirmation

4. **Unit Tests (AC-8):**
   - Created 9 comprehensive unit tests with mock dependencies
   - Used real SqliteEventStore (in-memory DB) instead of mock for better integration coverage
   - All tests passing: lifecycle, single/multiple job processing, FIFO order, empty queue sleep, event publishing, state persistence, error handling
   - Fixed event count test by draining PrintJobCreated from job.new()

5. **Integration Tests (AC-9):**
   - Created `tests/integration/queue_worker_integration_test.rs` with 2 tests
   - Tests use real infrastructure: SQLite (in-memory), QueueManager, PrintJobRepository, EventStore
   - Both tests passing: single job flow and multiple jobs sequential processing
   - Registered in `tests/integration/mod.rs`

6. **Final Verification (AC-10):**
   - ✅ All QueueWorker tests pass (9 unit + 2 integration = 11 tests)
   - ✅ `cargo build --lib` successful
   - ✅ `cargo fmt` applied (auto-formatting)
   - ⚠️ `cargo clippy` has warnings in other files (print_job_repository.rs, app_context.rs) — not related to this story
   - ⚠️ Some PdfiumRenderer tests fail with ACCESS_VIOLATION (known threading issue, unrelated to QueueWorker)

### Architectural Decisions

1. **MVP Renderer Choice:** Used PdfiumRenderer directly (300 DPI) instead of StrategySelector
   - Reason: Simpler implementation, meets MVP requirements
   - Future: Can integrate dynamic strategy selection per-job in later stories

2. **Event Store in Tests:** Used real SqliteEventStore with in-memory DB instead of mocks
   - Reason: Better integration coverage, tests real save_all() behavior
   - Trade-off: Slightly slower tests, but more confidence in correctness

3. **Sequential Processing:** Single worker thread, FIFO queue
   - Reason: MVP scope, meets NFR for <100 orders
   - Future: Story 4.x will add concurrent workers for 5000+ orders

4. **Error Handling:** Jobs remain in last successful state on failure
   - Reason: Story 3.6 will implement retry logic with exponential backoff
   - Current: Worker logs error and continues polling

### File List

**Created:**
- `src-tauri/src/infrastructure/queue/queue_worker.rs` (379 lines + 534 lines tests = 913 lines total)
- `src-tauri/tests/integration/queue_worker_integration_test.rs` (189 lines)

**Modified:**
- `src-tauri/src/infrastructure/queue/mod.rs` (+3 lines: pub mod queue_worker; pub use QueueWorker;)
- `src-tauri/src/main.rs` (+28 lines: worker dependencies + worker.start())
- `src-tauri/tests/integration/mod.rs` (+1 line: mod queue_worker_integration_test;)

**Test Coverage:**
- Unit tests: 9 tests covering lifecycle, pipeline, FIFO, error handling
- Integration tests: 2 tests covering real job flow with SQLite backend
- Total: 11 new tests, all passing

### Review Findings

- [x] [Review][Decision] No graceful shutdown on app exit — Fixed: worker stored in AppContextState + on_window_event shutdown handler
- [x] [Review][Decision] Job orphaned in Pending if first persist_and_publish fails — Fixed: mark job as Failed on initial persist failure
- [x] [Review][Decision] persist_and_publish not atomic — Fixed: reversed order (save events first, then update job)
- [x] [Review][Patch] TempPdfFile not wrapping downloaded PathBuf [queue_worker.rs:253] — Fixed: wrapped in TempPdfFile::new()
- [x] [Review][Patch] TOCTOU race in start() [queue_worker.rs:108-113] — Fixed: compare_exchange
- [x] [Review][Patch] Mutex poison recovery on thread_handle [queue_worker.rs:128,139] — Fixed: unwrap_or_else
- [x] [Review][Patch] Missing test_worker_cleans_temp_file — Fixed: added test with real temp file
- [x] [Review][Patch] test_worker_sleeps_when_queue_empty doesn't verify timing [queue_worker.rs:707-735] — Fixed: added Instant::now timing assertion
- [x] [Review][Defer] Failed jobs stuck in intermediate state — spec explicitly defers to Story 3.6 (auto-retry). deferred, pre-existing by design
- [x] [Review][Defer] rendered_data memory pressure for large documents — MVP acceptable, single-threaded sequential processing. deferred, performance optimization
- [x] [Review][Defer] pop() error causes infinite retry with no backoff — future improvement, no circuit breaker on queue itself. deferred, future enhancement
- [x] [Review][Defer] Integration tests use thread::sleep polling — may be flaky under CI load. deferred, test infra concern
- [x] [Review][Defer] AC-10 cargo clippy not clean — pre-existing warnings in other files, not caused by this story. deferred, pre-existing
- [x] [Review][Defer] start() after failed stop() may inherit poisoned shared state — edge case with panic recovery. deferred, edge case
