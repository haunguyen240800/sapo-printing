# Story 3.3: Implement CreatePrintJobUseCase with Event Publishing

Status: done

## Story

As a **nhân viên kho**,
I want **to create a print job via Tauri command**,
So that **I can initiate a print request from the UI and have it persisted with full event audit trail**.

## Context

Story này implement `CreatePrintJobUseCase` tại Application layer — use case đầu tiên kết nối domain, repository và event store theo **Outbox Pattern** (Architecture Decision 14).

**Business Value:**
- FR-1.1: Nhận print request (1-5000 URLs), validate, create job, return job_id
- FR-1.2: Job lifecycle bắt đầu từ đây — PENDING state với `PrintJobCreated` event
- AR-2: Event-Driven Architecture — events persist with aggregate trong cùng transaction, publish AFTER commit
- AR-14: Validation split — Application layer: technical validation; Domain layer: business rules

**What stories 3.6 + 3.7 provided (foundation đã có):**
- `SqlitePrintJobRepository` với save, update, find_by_id, find_by_status, find_all ✅
- `SqliteEventStore` với save_event, save_all (batch transaction), find_by_aggregate ✅
- `AppContext` với job_repo + event_store real implementations ✅
- Domain events: PrintJobCreated, PrintJobQueued, … với `serialize_payload()` ✅
- `PrintJob::new()` emit PrintJobCreated event, `drain_events()` drain event buffer ✅

**What this story does NOT do:**
- ❌ QueueManager (Story 3.4) | ❌ Download/Render pipeline (Stories 3.1–3.5 đã done) | ❌ UI (Story 3.8)
- ❌ Tauri `AppHandle` — command hiện dùng `State<AppContext>`, KHÔNG emit Tauri UI events (Story 3.9)

**Depends on:** Stories 3.6 ✅ + 3.7 ✅ + 1.2 ✅

## Acceptance Criteria

### AC-1: ApplicationError Enum

**Given** application layer cần error type riêng
**When** I create `src-tauri/src/application/use_cases/errors.rs`
**Then** phải định nghĩa theo pattern thủ công (KHÔNG dùng `thiserror` — **không có trong Cargo.toml**):

```rust
use std::fmt;
use crate::domain::print_job::errors::DomainError;

#[derive(Debug)]
pub enum ApplicationError {
    TooManyJobs { count: usize },
    EmptyJobList,
    PrinterNotAvailable { name: String },
    DomainError(DomainError),
    RepositoryError(String),
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyJobs { count } =>
                write!(f, "Số lượng URLs vượt quá giới hạn tối đa 5000 (nhận được: {})", count),
            Self::EmptyJobList =>
                write!(f, "Danh sách URLs không được rỗng"),
            Self::PrinterNotAvailable { name } =>
                write!(f, "Máy in '{}' không tồn tại hoặc không online", name),
            Self::DomainError(e) =>
                write!(f, "Lỗi domain: {}", e),
            Self::RepositoryError(msg) =>
                write!(f, "Lỗi lưu trữ: {}", msg),
        }
    }
}

impl std::error::Error for ApplicationError {}

impl From<DomainError> for ApplicationError {
    fn from(e: DomainError) -> Self {
        Self::DomainError(e)
    }
}
```

> **Lý do:** `Cargo.toml` hiện tại KHÔNG có `thiserror`. Pattern thủ công này nhất quán với `DomainError` và `EventBusError` đã có trong project.

### AC-2: CreateJobRequest DTO

**Given** use case cần input type rõ ràng
**When** I create request DTO
**Then** `src-tauri/src/application/dto/create_job_request.rs` phải có:

```rust
/// Input DTO cho CreatePrintJobUseCase.
/// Validation: 1–5000 URLs, printer_name không rỗng.
#[derive(Debug, Clone)]
pub struct CreateJobRequest {
    /// Danh sách S3 PDF URLs (1–5000)
    pub pdf_urls: Vec<String>,
    /// Tên máy in (phải match printer đang ONLINE)
    pub printer_name: String,
}
```

Export từ `application/dto/mod.rs`.

### AC-3: CreatePrintJobUseCase — Outbox Pattern

**Given** CreateJobRequest hợp lệ và printer ONLINE
**When** use case execute được gọi
**Then** `src-tauri/src/application/use_cases/create_print_job.rs` phải implement:

**Outbox Pattern (Architecture Decision 14):**
```
1. Validate request (Application layer)
2. Verify printer ONLINE (via printer_repo)
3. Tạo một PrintJob aggregate mỗi URL  [LƯU Ý: mỗi URL = 1 PrintJob riêng]
4. drain_events() từ mỗi job
5. job_repo.save() + event_store.save_all() — trong cùng logic (không phải DB transaction thực — xem Dev Notes)
6. event_bus.publish() MỖI event — CHỈ SAU KHI save thành công
7. Return Vec<JobId>
```

**Struct:**
```rust
pub struct CreatePrintJobUseCase {
    pub job_repo: Arc<dyn PrintJobRepository>,
    pub event_store: Arc<SqliteEventStore>,
    pub event_bus: Arc<dyn EventBus>,
    pub printer_repo: Arc<dyn PrinterRepository>,
}
```

**Signature:**
```rust
pub fn execute(&self, request: CreateJobRequest) -> Result<Vec<JobId>, ApplicationError>
```

**Validation rules (Application layer):**
- `pdf_urls.is_empty()` → `ApplicationError::EmptyJobList`
- `pdf_urls.len() > 5000` → `ApplicationError::TooManyJobs { count }`
- `printer_repo.find_by_name(&PrinterName::new(request.printer_name.clone()))` returns `None` → `ApplicationError::PrinterNotAvailable`
- Printer status != `PrinterStatus::Online` → `ApplicationError::PrinterNotAvailable`

> **CRITICAL:** `find_by_name` nhận `&PrinterName` (không phải `&str`). Phải wrap: `PrinterName::new(request.printer_name.clone())`. Import: `use crate::domain::printer::value_objects::PrinterName;`

### AC-4: Tauri Command `create_print_job`

**Given** use case exist
**When** I create Tauri command
**Then** `src-tauri/src/interface/tauri/commands/print_job.rs` phải có:

```rust
use std::sync::Arc;
use tauri::State;
use crate::application::dto::create_job_request::CreateJobRequest;
use crate::application::use_cases::{create_print_job::CreatePrintJobUseCase, errors::ApplicationError};

#[derive(serde::Deserialize)]
pub struct CreateJobPayload {
    pub pdf_urls: Vec<String>,
    pub printer_name: String,
}

#[tauri::command]
pub fn create_print_job(
    payload: CreateJobPayload,
    ctx: State<'_, AppContextState>,  // AppContextState — xem Dev Notes
) -> Result<Vec<String>, String> {
    let use_case = CreatePrintJobUseCase {
        job_repo: ctx.job_repo.clone(),
        event_store: ctx.event_store.clone(),
        event_bus: ctx.event_bus.clone(),
        printer_repo: ctx.printer_repo.clone(),
    };
    let request = CreateJobRequest {
        pdf_urls: payload.pdf_urls,
        printer_name: payload.printer_name,
    };
    use_case.execute(request)
        .map(|ids| ids.iter().map(|id| id.to_string()).collect())
        .map_err(|e| match &e {
            ApplicationError::EmptyJobList =>
                "Danh sách URLs không được rỗng".to_string(),
            ApplicationError::TooManyJobs { count } =>
                format!("Số lượng URLs vượt quá giới hạn 5000 (nhận được: {})", count),
            ApplicationError::PrinterNotAvailable { name } =>
                format!("Máy in '{}' không khả dụng hoặc đang offline", name),
            _ => format!("{}", e),
        })
}
```

> **Quan trọng:** Tauri State type là `AppContextState` (struct trong `main.rs`), KHÔNG phải `Arc<AppContext>`. `AppContext` struct có `todo!()` panic nên không dùng được.

### AC-5: Unit Tests — UseCase

**File:** inline `#[cfg(test)]` trong `create_print_job.rs`

Dùng **mock objects** — KHÔNG dùng SQLite trong unit tests:

```rust
// Mock PrintJobRepository
struct MockJobRepo { saved: Mutex<Vec<PrintJob>> }
impl PrintJobRepository for MockJobRepo {
    fn save(&self, job: &PrintJob) -> Result<(), DomainError> {
        self.saved.lock().unwrap().push(job.clone());
        Ok(())
    }
    // update, find_by_id, find_by_status, find_all → unimplemented!()
}

// Mock EventStore — không cần in-memory SQLite
struct MockEventStore; // implement save_event, save_all (Ok(()))

// Mock EventBus
struct TestEventBus { published: Arc<Mutex<Vec<(String, String)>>> }
impl EventBus for TestEventBus {
    fn publish(&self, event_type: &str, payload: &str) -> Result<(), EventBusError> {
        self.published.lock().unwrap().push((event_type.to_string(), payload.to_string()));
        Ok(())
    }
}

// Mock PrinterRepository
struct MockPrinterRepo { printer: Option<Printer> }
impl PrinterRepository for MockPrinterRepo {
    fn find_by_name(&self, _: &str) -> Result<Option<Printer>, PrinterDomainError> {
        Ok(self.printer.clone())
    }
    // save, find_all → unimplemented!()
}
```

**Required tests:**

1. **`test_valid_single_url_creates_job`** — 1 URL, printer ONLINE → Ok([job_id]), job saved, event published
2. **`test_valid_multiple_urls_creates_multiple_jobs`** — 3 URLs → Ok(3 job_ids), 3 jobs saved
3. **`test_empty_urls_returns_error`** — empty vec → `ApplicationError::EmptyJobList`
4. **`test_too_many_urls_returns_error`** — 5001 URLs → `ApplicationError::TooManyJobs { count: 5001 }`
5. **`test_exact_limit_5000_succeeds`** — 5000 URLs → Ok(5000 ids)
6. **`test_offline_printer_returns_error`** — printer OFFLINE → `ApplicationError::PrinterNotAvailable`
7. **`test_printer_not_found_returns_error`** — printer None → `ApplicationError::PrinterNotAvailable`
8. **`test_events_published_after_save`** — verify EventBus.publish called AFTER save (ordering)

### AC-6: Integration Test

`src-tauri/tests/integration/create_print_job_integration_test.rs` phải verify:
1. Real SQLite in-memory — create job → find_by_id returns job với status PENDING
2. Event stored in event_store — find_by_aggregate returns PrintJobCreated event
3. Printer ONLINE check — offline printer returns error, nothing saved

Register trong `tests/integration/mod.rs`.

### AC-7: Wire Command vào Tauri

**Given** command tạo xong
**When** I update `main.rs` và `lib.rs`
**Then:**
- `src-tauri/src/interface/tauri/commands/mod.rs` — export `print_job` module
- `src-tauri/src/interface/tauri/commands/print_job.rs` — tạo file
- `main.rs` hoặc `lib.rs` — thêm `create_print_job` vào `.invoke_handler(tauri::generate_handler![...])`

### AC-8: All Tests Pass

- `cargo test` — all pass (unit + integration)
- `cargo check` — zero errors
- `cargo clippy -- -D warnings` — clean
- `cargo fmt --check` — passes

## Tasks / Subtasks

- [ ] Task 0: Check Cargo.toml cho `thiserror` dependency
  - [ ] Nếu không có → implement ApplicationError thủ công (Display + Error)
  - [ ] Nếu có → dùng `#[derive(thiserror::Error)]`

- [ ] Task 1: ApplicationError (AC-1)
  - [ ] Tạo `src-tauri/src/application/use_cases/errors.rs`
  - [ ] Export từ `application/use_cases/mod.rs`

- [ ] Task 2: CreateJobRequest DTO (AC-2)
  - [ ] Tạo `src-tauri/src/application/dto/create_job_request.rs`
  - [ ] Export từ `application/dto/mod.rs`

- [ ] Task 3: CreatePrintJobUseCase (AC-3)
  - [ ] Tạo `src-tauri/src/application/use_cases/create_print_job.rs`
  - [ ] Implement validate → verify printer → create jobs → save+events → publish
  - [ ] Export từ `application/use_cases/mod.rs`

- [ ] Task 4: Unit tests (AC-5)
  - [ ] Mock types trong `#[cfg(test)]` của `create_print_job.rs`
  - [ ] 8 unit tests

- [ ] Task 5: Tauri command (AC-4 + AC-7)
  - [ ] Tạo `src-tauri/src/interface/tauri/commands/print_job.rs`
  - [ ] Update `commands/mod.rs`
  - [ ] Wire vào `tauri::generate_handler!` trong `lib.rs`/`main.rs`

- [ ] Task 6: Integration test (AC-6)
  - [ ] Tạo `tests/integration/create_print_job_integration_test.rs`
  - [ ] Register trong `tests/integration/mod.rs`

- [ ] Task 7: Final verification (AC-8)
  - [ ] `cargo test` | `cargo check` | `cargo clippy` | `cargo fmt`

## Dev Notes

### File Structure

```
src-tauri/src/application/
├── dto/
│   ├── mod.rs                    # UPDATE: pub mod create_job_request;
│   └── create_job_request.rs     # NEW: CreateJobRequest struct
├── use_cases/
│   ├── mod.rs                    # UPDATE: pub mod create_print_job; pub mod errors;
│   ├── errors.rs                 # NEW: ApplicationError enum
│   └── create_print_job.rs       # NEW: CreatePrintJobUseCase + unit tests
├── handlers/
│   └── mod.rs                    # UNCHANGED
├── services/
│   └── mod.rs                    # UNCHANGED
└── mod.rs                        # UNCHANGED

src-tauri/src/interface/tauri/commands/
├── mod.rs                        # UPDATE: pub mod print_job;
├── print_job.rs                  # NEW: create_print_job command
└── printer.rs                    # UNCHANGED

src-tauri/tests/integration/
├── mod.rs                        # UPDATE: mod create_print_job_integration_test;
└── create_print_job_integration_test.rs  # NEW
```

### Outbox Pattern — Hiểu đúng (CRITICAL)

Architecture Decision 14 nói "events persist with aggregate, publish AFTER commit". Tuy nhiên **SQLite connection của chúng ta dùng `Arc<Mutex<Connection>>`** — không có multi-statement transaction cross repositories.

**Thực tế implement:**

```rust
// Đây là pseudo-code của use case execute()
pub fn execute(&self, request: CreateJobRequest) -> Result<Vec<JobId>, ApplicationError> {
    // 1. Validate
    validate_request(&request)?;

    // 2. Verify printer — find_by_name nhận &PrinterName (không phải &str)
    let printer_name_vo = PrinterName::new(request.printer_name.clone());
    let printer = self.printer_repo
        .find_by_name(&printer_name_vo)
        .map_err(|e| ApplicationError::RepositoryError(e.to_string()))?
        .ok_or_else(|| ApplicationError::PrinterNotAvailable { name: request.printer_name.clone() })?;

    if *printer.status() != PrinterStatus::Online {
        return Err(ApplicationError::PrinterNotAvailable { name: request.printer_name.clone() });
    }

    // 3. Create jobs + collect events
    let mut all_job_ids = Vec::new();
    let mut all_events: Vec<Box<dyn DomainEvent>> = Vec::new();

    for url in &request.pdf_urls {
        let mut job = PrintJob::new(url.clone(), request.printer_name.clone());
        let events = job.drain_events();

        // 4. Save job FIRST
        self.job_repo.save(&job)
            .map_err(|e| ApplicationError::RepositoryError(e.to_string()))?;

        // 5. Save events to event store (best-effort persistence)
        self.event_store.save_all(job.id().to_string().as_str(), &events)
            .map_err(|e| ApplicationError::RepositoryError(e.to_string()))?;

        all_job_ids.push(job.id().clone());
        all_events.extend(events);
    }

    // 6. Publish AFTER all saves succeed
    for event in &all_events {
        let payload = event.serialize_payload();
        let _ = self.event_bus.publish(event.event_type(), &payload);
        // EventBus publish failures are non-fatal — log but don't fail
    }

    Ok(all_job_ids)
}
```

**Tại sao không phải single SQLite transaction cho cả batch:**
- `SqlitePrintJobRepository.save()` và `SqliteEventStore.save_all()` mỗi cái đều lock mutex riêng
- Batch 5000 URLs sẽ timeout nếu giữ lock quá lâu
- Per-URL atomic pair (save job + save events) là đủ cho MVP — Story 3.4 (QueueManager) sẽ xử lý ordering

### event_store.save_all() signature hiện tại

```rust
// Từ Story 3.7 implementation:
pub fn save_all(
    &self,
    aggregate_id: &str,
    events: &[Box<dyn DomainEvent>]
) -> Result<(), DomainError>
```

Dùng `job.id().to_string().as_str()` cho `aggregate_id`.

### EventBus.publish() signature

```rust
// Từ shared/event_bus.rs:
pub trait EventBus: Send + Sync {
    fn publish(&self, event_type: &str, payload: &str) -> Result<(), EventBusError>;
}
```

Gọi: `self.event_bus.publish(event.event_type(), &event.serialize_payload())`

### AppContextState Pattern trong main.rs — CRITICAL

**QUAN TRỌNG:** `main.rs` KHÔNG dùng `AppContext` struct. Nó dùng `AppContextState` inline struct riêng:

```rust
// Hiện tại trong main.rs (dòng 278-283):
struct AppContextState {
    printer_repo: Arc<dyn sapo_printer::domain::printer::PrinterRepository>,
    printer_manager: Arc<dyn PrinterManager>,
    _secret_manager: Arc<dyn SecretManager>,
    // job_repo và event_store CHƯA có — story này phải thêm vào
}
```

**Story này phải extend `AppContextState`** bằng cách thêm `job_repo`, `event_store`, `event_bus`:

```rust
// Sau khi story này xong:
struct AppContextState {
    printer_repo: Arc<dyn sapo_printer::domain::printer::PrinterRepository>,
    printer_manager: Arc<dyn PrinterManager>,
    _secret_manager: Arc<dyn SecretManager>,
    job_repo: Arc<dyn sapo_printer::domain::print_job::PrintJobRepository>,  // NEW
    event_store: Arc<sapo_printer::infrastructure::database::SqliteEventStore>,  // NEW
    event_bus: Arc<dyn sapo_printer::shared::event_bus::EventBus>,  // NEW
}
```

**Và wire vào `main()` (sau dòng 229):**
```rust
use sapo_printer::infrastructure::database::{SqliteEventStore, SqlitePrintJobRepository};
use sapo_printer::shared::event_bus::InMemoryEventBus;

let job_repo = Arc::new(SqlitePrintJobRepository::new(pool.get_arc()));
let event_store = Arc::new(SqliteEventStore::new(pool.get_arc()));
let event_bus: Arc<dyn sapo_printer::shared::event_bus::EventBus> =
    Arc::new(InMemoryEventBus::new());
```

**Commands đã có trong `main.rs` (KHÔNG phải trong commands/printer.rs):** `list_printers`, `save_printer_config`, `get_printer_status` đều được định nghĩa INLINE trong `main.rs`. Story này thêm `create_print_job` vào `main.rs` hoặc tạo module riêng và re-export.

### Integration Test — Setup pattern

Tạo dependencies trực tiếp (không qua AppContext):

```rust
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use sapo_printer::infrastructure::database::{
    migrations::run_migrations, SqliteEventStore, SqlitePrintJobRepository, SqlitePrinterRepository,
};
use sapo_printer::shared::event_bus::InMemoryEventBus;

fn setup_test_deps() -> (
    Arc<SqlitePrintJobRepository>,
    Arc<SqliteEventStore>,
    Arc<InMemoryEventBus>,
    Arc<SqlitePrinterRepository>,
) {
    let mut conn = Connection::open_in_memory().unwrap();
    run_migrations(&mut conn).unwrap();  // CRITICAL: phải chạy migrations trước
    let arc_conn = Arc::new(Mutex::new(conn));
    (
        Arc::new(SqlitePrintJobRepository::new(arc_conn.clone())),
        Arc::new(SqliteEventStore::new(arc_conn.clone())),
        Arc::new(InMemoryEventBus::new()),
        Arc::new(SqlitePrinterRepository::new(arc_conn.clone())),
    )
}
```

**Seed printer ONLINE trước mỗi test:**
```rust
use sapo_printer::domain::printer::{Printer, PrinterName, PrinterType};
use sapo_printer::domain::printer::PrinterRepository;

let (job_repo, event_store, event_bus, printer_repo) = setup_test_deps();
let printer = Printer::new(
    PrinterName::new("HP_Test".to_string()),
    PrinterType::Local,
);
printer_repo.save(&printer).unwrap();
// Printer mới tạo có status ONLINE by default — kiểm tra domain/printer/aggregate.rs
```

### PrinterRepository trait — find_by_name

```rust
// Từ domain/printer/repository.rs (thực tế):
pub trait PrinterRepository: Send + Sync {
    fn save(&self, printer: &Printer) -> Result<(), PrinterDomainError>;
    fn find_all(&self) -> Result<Vec<Printer>, PrinterDomainError>;
    fn find_by_name(&self, name: &PrinterName) -> Result<Option<Printer>, PrinterDomainError>;
    //                              ^^^^^^^^^^^ PrinterName, KHÔNG phải &str
}
```

**Cách gọi đúng trong use case:**
```rust
use crate::domain::printer::value_objects::PrinterName;

let printer_name_vo = PrinterName::new(request.printer_name.clone());
let printer = self.printer_repo
    .find_by_name(&printer_name_vo)
    .map_err(|e| ApplicationError::RepositoryError(e.to_string()))?
    .ok_or_else(|| ApplicationError::PrinterNotAvailable {
        name: request.printer_name.clone()
    })?;
```

`find_by_name` trả `Result<Option<Printer>, PrinterDomainError>`. Map error thành `ApplicationError::RepositoryError`.

### Printer::status() và PrinterStatus::Online

```rust
// Từ domain/printer/value_objects.rs:
pub enum PrinterStatus { Online, Offline, Error }

// Từ domain/printer/aggregate.rs — dùng accessor:
printer.status() -> &PrinterStatus
// So sánh:
*printer.status() == PrinterStatus::Online
```

### Mỗi URL = 1 PrintJob riêng

```rust
for url in &request.pdf_urls {
    let mut job = PrintJob::new(url.clone(), request.printer_name.clone());
    // ...
}
```

Không group URLs vào một job. Story 3.4 (QueueManager) sẽ xử lý batch processing.

### Imports cần thiết

```rust
// create_print_job.rs
use std::sync::Arc;
use crate::domain::print_job::{
    aggregate::PrintJob,
    repository::PrintJobRepository,
    value_objects::JobId,
};
use crate::domain::printer::{
    repository::PrinterRepository,
    value_objects::{PrinterName, PrinterStatus},  // PrinterName cho find_by_name
};
use crate::infrastructure::database::SqliteEventStore;
use crate::shared::event_bus::EventBus;
use crate::application::dto::create_job_request::CreateJobRequest;
use crate::application::use_cases::errors::ApplicationError;
```

> **Lưu ý:** Không cần import `DomainError` trực tiếp vì `From<DomainError>` đã impl trong `ApplicationError`.

### Pattern từ Story 3.7 — setup_test_db cho unit test event store mock

Unit tests KHÔNG dùng real `SqliteEventStore`. Thay vào đó implement `MockEventStore`:

```rust
#[cfg(test)]
struct MockEventStore;

#[cfg(test)]
impl MockEventStore {
    // Cần implement các method của SqliteEventStore
    // Nhưng SqliteEventStore là concrete struct, không phải trait
    // → Cần tạo trait EventStorePort hoặc dùng Arc<dyn Any>
}
```

**QUAN TRỌNG:** `SqliteEventStore` là **concrete struct**, không phải trait. Use case cần `Arc<SqliteEventStore>` (không phải `Arc<dyn EventStore>`). Điều này làm unit testing khó hơn.

**Giải pháp đơn giản nhất:** Tạo `EventStore` trait mới:

```rust
// src-tauri/src/infrastructure/database/event_store.rs — THÊM trait
pub trait EventStoreTrait: Send + Sync {
    fn save_all(&self, aggregate_id: &str, events: &[Box<dyn DomainEvent>]) -> Result<(), DomainError>;
}

impl EventStoreTrait for SqliteEventStore {
    fn save_all(&self, aggregate_id: &str, events: &[Box<dyn DomainEvent>]) -> Result<(), DomainError> {
        self.save_all(aggregate_id, events)
    }
}
```

Sau đó UseCase dùng `Arc<dyn EventStoreTrait>` thay vì `Arc<SqliteEventStore>`.

**Hoặc giải pháp đơn giản hơn:** Giữ `Arc<SqliteEventStore>` trong UseCase, và trong unit test dùng real in-memory SQLite (dùng `setup_test_db()` pattern từ Story 3.7). Unit test sẽ nặng hơn một chút nhưng không cần thêm trait.

→ **Khuyến nghị:** Dùng real in-memory SQLite cho event store trong cả unit lẫn integration test. Nhanh hơn việc tạo trait mới.

### Cargo.toml — Dependencies đã verified

```toml
# Đã confirmed từ Cargo.toml thực tế:
# thiserror = KHÔNG CÓ  → implement ApplicationError thủ công (đã làm trong AC-1)
serde_json = "1"    # ✅ có
rusqlite = "0.32"   # ✅ có (bundled)
uuid = "1"          # ✅ có (v4 + serde)
tokio = "1"         # ✅ có
tracing = "0.1"     # ✅ có
```

> **KHÔNG thêm `thiserror`** — dùng pattern thủ công như AC-1 đã định nghĩa.

### main.rs — Tauri generate_handler! pattern thực tế

**QUAN TRỌNG:** `lib.rs` chỉ declare modules, KHÔNG có `invoke_handler`. Handler registration nằm trong **`main.rs` dòng 265-269**:

```rust
// main.rs hiện tại:
.invoke_handler(tauri::generate_handler![
    list_printers,
    save_printer_config,
    get_printer_status
])
```

**Thêm `create_print_job` vào đây** — function này phải accessible từ `main.rs`. Có 2 cách:
1. Define inline trong `main.rs` (nhất quán với 3 commands hiện tại)
2. Import từ module: `use sapo_printer::interface::tauri::commands::print_job::create_print_job;`

**Khuyến nghị cách 2** để tách biệt code, nhưng cần re-export đúng từ `interface/tauri/commands/mod.rs`.

## Previous Story Learnings

### From Story 3.7 (SqlitePrintJobRepository):

- `setup_test_db()` pattern: `Connection::open_in_memory()` + `run_migrations()` + `Arc::new(Mutex::new(conn))`
- `unwrap_or_else(|p| p.into_inner())` cho mutex poisoned handling
- `SqlitePrintJobRepository::new(conn: Arc<Mutex<Connection>>)` constructor
- `SqliteEventStore::save_all()` lock mutex TRƯỚC transaction
- `DomainEvent` trait có `serialize_payload()` method ✅
- `DomainError` variants: RepositoryError, InvalidStatus, InvalidJobId ✅

### From Story 3.6 (Schema):

- `print_jobs` table: `CHECK(length(id) = 36)` — JobId phải là UUID v4 string 36 chars
- `events` table: `UNIQUE(aggregate_id, sequence_number)` — sequence tự increment

### From Stories 2.1 (Database):

- `DbPool::new(db_path)` → `Result<DbPool, DatabaseError>`
- `pool.get()` → `MutexGuard<Connection>`, `pool.get_arc()` → `Arc<Mutex<Connection>>`

### From Story 1.4 (AppContext):

- `AppContext::new()` hiện panic vì `todo!("PrinterManager initialization")`
- Integration tests bypass AppContext::new() bằng cách tạo dependencies thủ công

### Environment Note (từ Story 3.7):

`cargo build` full có thể bị block bởi Windows Defender Application Control (WDAC) policy trên machine này — `os error 4551`. Đây là pre-existing environment issue. Dùng `cargo check` để verify compilation thay vì `cargo build`.

## References

- `epics.md` — Story 3.3 ACs (lines 921–935)
- `architecture.md` — Decision 14 (Outbox Pattern), Decision 16 (Validation split)
- `src-tauri/src/domain/print_job/aggregate.rs` — PrintJob::new(), drain_events()
- `src-tauri/src/domain/print_job/repository.rs` — PrintJobRepository trait
- `src-tauri/src/domain/print_job/events.rs` — DomainEvent trait + serialize_payload()
- `src-tauri/src/domain/printer/repository.rs` — PrinterRepository trait
- `src-tauri/src/domain/printer/value_objects.rs` — PrinterStatus::Online
- `src-tauri/src/infrastructure/database/event_store.rs` — SqliteEventStore + save_all()
- `src-tauri/src/infrastructure/database/print_job_repository.rs` — SqlitePrintJobRepository
- `src-tauri/src/shared/app_context.rs` — AppContext fields (job_repo, event_store, event_bus, printer_repo)
- `src-tauri/src/shared/event_bus.rs` — EventBus trait + InMemoryEventBus
- `src-tauri/src/interface/tauri/commands/printer.rs` — Tauri command pattern
- `_bmad-output/implementation-artifacts/3-7-implement-sqlite-print-job-repository.md` — previous story

## Dev Agent Record

### Agent Model Used
(to be filled)

### Completion Notes List
(to be filled)

### File List
(to be filled)
