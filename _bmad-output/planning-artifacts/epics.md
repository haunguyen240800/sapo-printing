---
stepsCompleted: ["step-01-validate-prerequisites", "step-02-design-epics", "step-03-create-stories"]
inputDocuments: 
  - "_bmad-output/planning-artifacts/prds/prd-sapo-printer-2026-06-22/prd.md"
  - "_bmad-output/planning-artifacts/architecture.md"
---

# sapo-printer - Epic Breakdown

## Overview

This document provides the complete epic and story breakdown for sapo-printer, decomposing the requirements from the PRD, UX Design if it exists, and Architecture requirements into implementable stories.

## Requirements Inventory

### Functional Requirements

**FR-1: Bulk Print Management**

FR-1.1: Nhận Print Request từ Web App
- System nhận print command qua Native Messaging
- Request gồm: `pdf_urls[]` (S3 links), `printer_name`, `config` (paper, margins, color)
- Validate: số lượng (1-5000), URLs hợp lệ, printer online
- Trả về `job_id` hoặc error

FR-1.2: Tạo và Quản lý Print Jobs
- Job States: PENDING → QUEUED → DOWNLOADED → SUBMITTED_TO_QUEUE → PRINTING → COMPLETED (hoặc FAILED với retry)
- State definitions: PENDING (mới tạo), QUEUED (chờ xử lý), DOWNLOADED (PDF đã download), SUBMITTED_TO_QUEUE (đẩy vào print queue), PRINTING (đang in), COMPLETED (hoàn thành), FAILED (thất bại, retry nếu < 3 lần)

FR-1.3: Batch Processing & Queue
- Khi ≥100 URLs → bật chế độ "In số lượng lớn"
- Persist: Lưu tất cả jobs vào database (SQLite)
- Queue: Jobs vào durable queue, xử lý tuần tự
- Download batch: Download song song (max 10 concurrent), lưu temp files
- Print sequential: Xử lý print tuần tự từ queue
- Cleanup: Xóa temp file sau khi COMPLETED hoặc FAILED (sau retry cuối)

FR-1.4: Auto-Retry Logic
- Khi job FAILED, kiểm tra retry_count < 3
- Nếu có thể retry: đẩy lại vào QUEUED, exponential backoff (5s → 10s → 20s)
- Retry cho: download timeout, render error, printer temporary error
- Không retry: validation errors, 404 URLs
- Sau 3 lần → giữ FAILED permanently

FR-1.5: Job Cancellation
- Cancel được ở: PENDING, QUEUED, DOWNLOADED, SUBMITTED_TO_QUEUE
- PRINTING có thể cancel với warning
- Không cancel: COMPLETED, FAILED
- Cleanup temp file khi cancel

**FR-2: Printer Management**

FR-2.1: Printer Discovery (Cross-Platform)
- Tự động phát hiện tất cả máy in khả dụng
- Windows: Win32 EnumPrinters API
- macOS/Linux: CUPS (lpstat hoặc CUPS API)
- Return danh sách máy in với: tên, trạng thái, loại
- Nếu không có máy in → return empty list

FR-2.2: Printer Status Monitoring
- Kiểm tra trạng thái: ONLINE, OFFLINE, ERROR
- Cập nhật định kỳ (mỗi 5 giây khi app active)
- Hiển thị thông báo khi máy in offline/error

FR-2.3: Printer Configuration
- Basic Settings: Chọn máy in (Dropdown list), Paper Size (A4/A5/Letter/Custom), Custom dimensions (Width × Height mm), In chiều ngang (Checkbox)
- Print Mode: In ảnh (Checkbox), Loại ảnh in: RGB/ARGB/BGR/GRAY/BINARY (hiện khi "In ảnh" ON)
- Layout: Margins - Left, Right, Top, Bottom (mm)
- Advanced: Bật Printing Buffer (Checkbox, default OFF), Buffer Size (KB, hiện khi buffer được bật)

FR-2.4: Default Printer Selection
- First launch → tự động lấy máy in mặc định từ OS
- Empty list → UI hiển thị cảnh báo

FR-2.5: Printer Capability Detection
- Detect Direct PDF support
- Chọn strategy: Direct PDF (fast) hoặc Render (control)

**FR-3: Document Processing**

FR-3.1: Document Download
- Download PDF từ S3 URLs
- Timeout: 30 giây
- Lưu temp directory
- Validate PDF header

FR-3.2: PDF Rendering Strategy (Hybrid)
- Direct PDF: ~0.5s (fast)
- Render: ~2-3s (control)
- Tự động chọn strategy

FR-3.3: Color Mode Conversion
- Hỗ trợ 5 modes: RGB (24-bit), ARGB (32-bit với alpha), BGR (Windows default), GRAY (8-bit grayscale), BINARY (1-bit monochrome)

FR-3.4: Margins Application
- Apply margins (mm) khi render
- Convert mm → pixels at 300 DPI

FR-3.5: Paper Size Handling
- Predefined: A4, A5, Letter
- Custom: 50-500mm
- Internal: mm

**FR-4: User Interface**

FR-4.1: Print Status Dashboard
- Thông tin cấu hình: printer, paper, margins, color
- Tiến trình: download, print progress
- Thời gian: elapsed + estimated
- Metrics: tổng/thành công/thất bại
- Job list với filters

FR-4.2: Printer Configuration Screen
- 5 sections: Printer Selection, Paper Settings, Print Mode, Layout, Advanced

FR-4.3: Auto-Update Popup
- Auto check khi startup
- Hiển thị: app name, version, website
- Actions: Cập nhật, Đóng, Kiểm tra phiên bản

FR-4.4: Error Notification & Recovery
- Toast notifications
- Error detail modal với retry button

**FR-5: System Integration**

FR-5.1: Native Messaging
- Desktop đăng ký host với browser
- JSON protocol: ping, print_batch, get_status, cancel_job, list_printers

FR-5.2: Status Sync
- v1: Polling mỗi 2s
- v2: WebSocket real-time (future)

FR-5.3: Configuration Persistence
- SQLite: printer_configs, print_jobs, app_settings
- Backup trước update

FR-5.4: Auto-Update
- Check: startup + 24h
- Tauri Updater plugin
- Platform: NSIS/DMG/AppImage

**FR-6: Audit & Logging**

FR-6.1: Audit Trail
- Log tất cả jobs với full context
- Retention: 30 ngày

FR-6.2: Error Logging
- Structured logs: DEBUG/INFO/WARN/ERROR
- Rotation: daily, 7 ngày

FR-6.3: Metrics Collection
- Job metrics, queue metrics, printer metrics, performance
- Export qua UI

### NonFunctional Requirements

**NFR-1: Performance**

- Throughput: Xử lý tối thiểu 100 đơn/phút (trung bình), Batch 5000 đơn hoàn thành trong < 60 phút
- Latency: Printer discovery < 2s, PDF download < 30s per document, PDF rendering < 1s per page (300 DPI, A4), Direct PDF print ~0.5s per job, Render strategy print ~2-3s per job, UI response time < 200ms
- Resource Usage: Memory < 500MB khi xử lý 5000 đơn, CPU < 70% during peak load, Disk cleanup temp files ngay sau job complete, Concurrent downloads: Max 10 parallel, Concurrent renders: Max 10 parallel
- App Startup: Cold start < 3s từ launch đến UI ready

**NFR-2: Reliability & Availability**

- Success Rate: Print job success rate ≥ 99%, Auto-retry success ≥ 80% các job lỗi tạm thời được recover
- Uptime: App chạy liên tục 24/7 không crash, Graceful degradation khi mất network
- Data Durability: Jobs persist qua app restart (durable queue), Config backup trước khi update, Audit trail không bị mất
- Error Recovery: Auto-retry với exponential backoff, Không crash khi gặp corrupt PDF, Timeout protection cho tất cả network calls

**NFR-3: Usability**

- Ease of Use: Người dùng mới sử dụng được trong < 5 phút, UI tiếng Việt, rõ ràng, trực quan
- Feedback & Transparency: Real-time status updates (mỗi 2s), Progress indicators cho download/print, Error messages rõ ràng, actionable
- Accessibility: Keyboard navigation support, High contrast mode (optional v2)

### Additional Requirements

**Architecture Requirements:**

AR-1: Clean Architecture + DDD Pattern
- 4 layers: Interface → Application → Domain → Infrastructure
- Domain layer hoàn toàn độc lập — không dependencies nào
- Dependency Inversion Principle bắt buộc

AR-2: Event-Driven Architecture
- 8+ Domain Events cho mọi state transition: PrintJobCreated, PrintJobQueued, PrintJobDownloaded, PrintJobSubmitted, PrintJobCompleted, PrintJobFailed, PrinterConnected, PrinterDisconnected
- Event Bus với ordering support
- Event Store cho audit trail (30 ngày retention)
- Outbox Pattern: Events persist with aggregate, publish only after successful commit

AR-3: Cross-Platform Abstraction
- Trait-based abstraction cho Win32 vs CUPS
- Platform-specific implementations: WindowsPrinterManager, CupsPrinterManager, WindowsPrinterEngine, CupsPrinterEngine
- Conditional compilation: #[cfg(target_os = "windows")], #[cfg(unix)]

AR-4: Repository Pattern
- PrintJobRepository trait, PrinterRepository trait
- SQLite implementations: SqlitePrintJobRepository, SqlitePrinterRepository
- WAL mode cho concurrent reads
- Connection pooling strategy

AR-5: Strategy Pattern
- DocumentRenderer trait với multiple implementations
- Hybrid rendering: Direct PDF (~0.5s) vs MuPDF Render (~2-3s)
- Automatic strategy selection based on printer capabilities

AR-6: Queue Management (Infrastructure Concern)
- Durable queue (SQLite-backed)
- Worker pattern với batch processing
- Auto-retry với exponential backoff
- Circuit breaker for S3 downloads

AR-7: RAII Pattern for Resource Cleanup
- TempPdfFile struct với Drop trait
- Automatic cleanup khi out of scope
- Tiered cleanup strategy: Immediate (success), Deferred (failed jobs 24h), Startup cleanup (orphaned files)

AR-8: Database Schema & Migrations
- rusqlite_migration library for versioning
- Schemas: print_jobs, events, printer_configs, app_settings
- Migration runs at app startup — zero manual steps

AR-9: Secret Management
- OS Keychain/Credential Manager for device tokens and HMAC keys
- Windows: Credential Manager API
- macOS: Keychain Services
- Linux: Secret Service API (libsecret)

AR-10: Dependency Injection Pattern
- Manual Constructor DI with AppContext wrapper
- Single initialization point (main.rs)
- Type-safe dependency injection
- Easy testing via MockAppContext

AR-11: Tauri Integration
- React Context API for state management
- Event-Driven UI Updates via Tauri events (tauri::Manager::emit)
- Typed Result<T, AppError> for command responses
- Real-time updates: job_status_changed, printer_status_changed, download_progress, print_progress

AR-12: Code Signing & Auto-Update
- Full code signing: Windows (EV cert), macOS (Apple Developer ID + Notarization), Linux (GPG signing)
- Tauri Built-in Updater: Check on startup + every 24h
- Update manifest hosted on GitHub Releases

AR-13: Logging & Observability
- Structured logging với tracing crate
- Logging levels: Development (DEBUG), Production (INFO)
- Log rotation: Daily, 7 days retention
- Local logs only (privacy-first, no external telemetry)

AR-14: Error Handling Strategy
- 3-tier errors: ApplicationError, DomainError, InfrastructureError
- Retry logic với exponential backoff
- Timeout protection cho external calls
- Dead Letter Queue for failed event handlers

AR-15: Testing Strategy
- Unit tests: Mock trait implementations (fast, isolated), Inline tests với #[cfg(test)]
- Integration tests: In-memory SQLite (:memory:)
- E2E tests: Test SQLite file với cleanup
- Cross-platform: GitHub Actions matrix (Windows + Linux + macOS runners)

AR-16: Project Initialization
- Starter approach: Manual Setup on Official Tauri Base (create-tauri-app)
- No suitable template exists for DDD + Clean Architecture + @sapo/ui-components
- Manual setup ensures correct structure from day 1

### UX Design Requirements

(Không có UX Design document — section này để trống)

### FR Coverage Map

**Epic 1 - Project Foundation & Core Domain:**
- AR-1: Clean Architecture + DDD Pattern
- AR-2: Event-Driven Architecture
- AR-10: Dependency Injection Pattern
- AR-16: Project Initialization

**Epic 2 - Cross-Platform Printer Infrastructure:**
- FR-2.1: Printer Discovery (Cross-Platform)
- FR-2.2: Printer Status Monitoring
- FR-2.3: Printer Configuration
- FR-2.4: Default Printer Selection
- FR-2.5: Printer Capability Detection
- AR-3: Cross-Platform Abstraction
- AR-4: Repository Pattern
- AR-8: Database Schema & Migrations
- AR-9: Secret Management

**Epic 3 - Bulk Print Job Management with Document Processing Pipeline:**
- FR-1.1: Nhận Print Request từ Web App
- FR-1.2: Tạo và Quản lý Print Jobs
- FR-1.3: Batch Processing & Queue
- FR-1.4: Auto-Retry Logic
- FR-1.5: Job Cancellation
- FR-3.1: Document Download
- FR-3.2: PDF Rendering Strategy (Hybrid)
- FR-3.3: Color Mode Conversion
- FR-3.4: Margins Application
- FR-3.5: Paper Size Handling
- FR-4.1: Print Status Dashboard
- FR-4.4: Error Notification & Recovery
- AR-5: Strategy Pattern
- AR-6: Queue Management (Durable Queue + Circuit Breaker)
- AR-7: RAII Pattern for Resource Cleanup
- AR-11: Tauri Integration
- AR-14: Error Handling Strategy
- NFR-1: Performance
- NFR-2: Reliability & Availability

**Epic 4 - System Integration & Production Readiness:**
- FR-5.1: Native Messaging
- FR-5.2: Status Sync
- FR-5.3: Configuration Persistence
- FR-5.4: Auto-Update
- FR-6.1: Audit Trail
- FR-6.2: Error Logging
- FR-6.3: Metrics Collection
- FR-4.3: Auto-Update Popup
- AR-11: Tauri Integration
- AR-12: Code Signing & Auto-Update
- AR-13: Logging & Observability
- NFR-3: Usability

## Epic List

### Epic 1: Project Foundation & Core Domain

⚠️ **SETUP EPIC (NON-DELIVERABLE)** — Epic này là greenfield project initialization và KHÔNG deliver user-facing value. Phải hoàn thành trước khi bắt đầu bất kỳ epic nào khác. Không treat như shippable milestone.

Team có được project structure chuẩn DDD + Clean Architecture, domain model hoàn chỉnh (aggregates, events, repository traits), sẵn sàng implement business logic.

**FRs covered:** AR-1 (Clean Architecture), AR-2 (Event-Driven), AR-10 (DI Pattern), AR-16 (Project Init)

**Implementation Notes:** Tauri project initialization + 4-layer structure + Domain layer (không có external dependencies)

**Dependencies:** None — must complete first before any other epic

---

### Epic 2: Cross-Platform Printer Infrastructure

Nhân viên kho có thể discover và monitor máy in (Windows/macOS/Linux), configure printer settings (paper, margins, color), app tự động detect máy in mặc định.

**FRs covered:** FR-2.1, FR-2.2, FR-2.3, FR-2.4, FR-2.5, AR-3 (Cross-Platform), AR-4 (Repository), AR-8 (Database), AR-9 (Secret Management)

**Implementation Notes:** Win32/CUPS printer discovery + status monitoring + configuration UI + SQLite persistence

---

### Epic 3: Bulk Print Job Management with Document Processing Pipeline

Nhân viên kho có thể in hàng loạt phiếu (1-5000 đơn) với một click. App tự động download PDFs từ S3, render với hybrid strategy (Direct PDF hoặc MuPDF), apply margins/color modes, batch processing, auto-retry khi lỗi, cancel jobs, cleanup temp files tự động, và hiển thị real-time status dashboard.

**FRs covered:** FR-1.1, FR-1.2, FR-1.3, FR-1.4, FR-1.5, FR-3.1, FR-3.2, FR-3.3, FR-3.4, FR-3.5, FR-4.1, FR-4.4, AR-5 (Strategy Pattern), AR-6 (Queue Management + Circuit Breaker), AR-7 (RAII Cleanup), AR-11 (Tauri Integration), AR-14 (Error Handling), NFR-1 (Performance), NFR-2 (Reliability)

**Implementation Notes:** Document processing pipeline (download, hybrid rendering, color conversion, temp file management) + Print job aggregate + Durable queue + Auto-retry logic + Real-time UI updates + Event handlers

**Dependencies:** Epic 1 (Foundation), Epic 2 (Printer Infrastructure)

---

### Epic 4: System Integration & Production Readiness

Desktop app tích hợp với web app qua Native Messaging, tự động update khi có version mới, có audit trail đầy đủ, metrics collection, production-ready deployment.

**FRs covered:** FR-5.1, FR-5.2, FR-5.3, FR-5.4, FR-6.1, FR-6.2, FR-6.3, FR-4.3, AR-11 (Tauri), AR-12 (Code Signing + Auto-Update), AR-13 (Logging), NFR-3 (Usability)

**Implementation Notes:** Native Messaging protocol + Status sync + Auto-update + Audit logging + Metrics + Code signing

---

## Epic 1: Project Foundation & Core Domain

Team có được project structure chuẩn DDD + Clean Architecture, domain model hoàn chỉnh (aggregates, events, repository traits), sẵn sàng implement business logic.

### Story 1.1: Initialize Tauri Project with 4-Layer Structure

As a **developer**,
I want **to initialize a Tauri v2 project with complete 4-layer Clean Architecture structure**,
So that **the team has a standardized foundation following DDD principles and can start implementing domain logic immediately**.

**Acceptance Criteria:**

**Given** the project repository is empty
**When** I run the Tauri initialization commands
**Then** the project structure must include:
- Tauri v2 base scaffolding (React 18 + TypeScript + Vite 7+ + pnpm)
- Frontend dependencies: @sapo/ui-components ^2.19.0, @sapo/ui-icons ^1.19.0, @emotion/react ^11.14.0, react-hook-form ^7.79.0, yup ^1.7.1
- 4-layer Rust backend structure:
  - `src-tauri/src/interface/` (empty, ready for Tauri commands)
  - `src-tauri/src/application/` (empty, ready for use cases)
  - `src-tauri/src/domain/` (empty, ready for aggregates)
  - `src-tauri/src/infrastructure/` (empty, ready for implementations)
  - `src-tauri/src/shared/` (empty, ready for cross-cutting concerns)
- Rust dependencies in Cargo.toml: rusqlite, tokio, tracing, uuid, serde
**And** the project must compile successfully with `cargo build`
**And** the development server must start with `cargo tauri dev`

### Story 1.2: Create PrintJob Domain Model

As a **developer**,
I want **to implement the complete PrintJob domain model (aggregate, events, value objects, repository trait)**,
So that **the core business logic for print job management is defined and ready for infrastructure implementation**.

**Acceptance Criteria:**

**Given** the domain layer structure exists
**When** I implement the PrintJob domain model
**Then** the following must be created in `src-tauri/src/domain/print_job/`:
- `value_objects.rs`: JobId (UUID), PrintStatus enum (PENDING, QUEUED, DOWNLOADED, SUBMITTED_TO_QUEUE, PRINTING, COMPLETED, FAILED)
- `events.rs`: PrintJobCreated, PrintJobQueued, PrintJobDownloaded, PrintJobSubmitted, PrintJobCompleted, PrintJobFailed domain events
- `aggregate.rs`: PrintJob aggregate với business rules:
  - Cannot retry if retry_count >= 3
  - Cannot cancel if status is COMPLETED or FAILED
  - State transitions must publish corresponding events
- `repository.rs`: PrintJobRepository trait with methods: save(), update(), find_by_id()
**And** all structs must implement necessary traits (Clone, Debug, Serialize, Deserialize)
**And** inline unit tests (`#[cfg(test)]`) must cover:
  - JobId generation is unique
  - PrintStatus state transitions are valid
  - PrintJob business rules (max retry, cannot cancel completed)
  - Domain events are collected correctly
**And** all tests must pass with `cargo test`
**And** the domain layer must have ZERO external dependencies (no infrastructure, no application layer imports)

### Story 1.3: Create Printer Domain Model

As a **developer**,
I want **to implement the complete Printer domain model (aggregate, events, value objects, repository trait)**,
So that **printer management business logic is defined independently and can be implemented across platforms**.

**Acceptance Criteria:**

**Given** the domain layer structure exists
**When** I implement the Printer domain model
**Then** the following must be created in `src-tauri/src/domain/printer/`:
- `value_objects.rs`: PrinterId, PrinterName, PrinterStatus enum (ONLINE, OFFLINE, ERROR), PrinterType
- `events.rs`: PrinterConnected, PrinterDisconnected domain events
- `aggregate.rs`: Printer aggregate với business rules:
  - Only ONLINE printers can receive print jobs
  - Printer status must be validated before job assignment
- `repository.rs`: PrinterRepository trait with methods: save(), find_all(), find_by_name()
**And** all structs must implement necessary traits (Clone, Debug, Serialize, Deserialize)
**And** inline unit tests (`#[cfg(test)]`) must cover:
  - PrinterStatus validation logic
  - Business rule: cannot assign job to offline printer
  - Domain events are published on status change
**And** all tests must pass with `cargo test`
**And** the domain layer remains independent (no external dependencies)

### Story 1.4: Create Document Domain Model & AppContext DI Container

As a **developer**,
I want **to implement the Document domain model and create the AppContext dependency injection container**,
So that **all domain models are complete and the application has a centralized DI pattern ready for use cases**.

**Acceptance Criteria:**

**Given** PrintJob and Printer domain models exist
**When** I implement Document domain and AppContext
**Then** the following must be created:

**Document Domain** (`src-tauri/src/domain/document/`):
- `value_objects.rs`: DocumentId, DocumentType enum (PDF), DocumentLocation (URL)
- `aggregate.rs`: Document aggregate với validation rules for PDF URLs

**AppContext DI Container** (`src-tauri/src/shared/app_context.rs`):
- AppContext struct with placeholder fields for:
  - `job_repo: Arc<dyn PrintJobRepository>`
  - `printer_repo: Arc<dyn PrinterRepository>`
  - `event_bus: Arc<dyn EventBus>` (trait defined in shared/)
- AppContext::new() constructor accepting database path
- Platform-specific printer engine selection:
  - `#[cfg(target_os = "windows")]` returns placeholder for WindowsPrinterEngine
  - `#[cfg(not(target_os = "windows"))]` returns placeholder for CupsPrinterEngine

**And** EventBus trait must be defined in `src-tauri/src/shared/event_bus.rs`:
- `publish<E: DomainEvent>(event: E) -> Result<()>`

**And** inline unit tests must verify:
- Document validation rejects invalid URLs
- AppContext initializes with correct platform-specific engine marker

**And** all tests must pass with `cargo test`
**And** compilation must succeed for all target platforms (Windows/macOS/Linux conditional compilation)


---

## Epic 2: Cross-Platform Printer Infrastructure

Nhân viên kho có thể discover và monitor máy in (Windows/macOS/Linux), configure printer settings (paper, margins, color), app tự động detect máy in mặc định.

### Story 2.1: Setup SQLite Database with Migrations & Schemas

As a **developer**,
I want **to setup SQLite database with migration system and create printer-related schemas**,
So that **the application can persist printer configurations and the database can evolve safely across versions**.

**Acceptance Criteria:**

**Given** the infrastructure layer structure exists
**When** I implement the database setup
**Then** the following must be created in `src-tauri/src/infrastructure/database/`:
- `connection.rs`: SQLite connection with WAL mode enabled, connection pooling setup
- `migrations.rs`: rusqlite_migration integration with initial migrations:
  - Migration 1: Create `printer_configs` table (id, printer_name, device_id UNIQUE, paper_size, paper_width, paper_height, orientation, margin_left, margin_right, margin_top, margin_bottom, color_mode, print_as_image, enable_buffer, buffer_size_kb, is_default, created_at, updated_at)
  - Migration 1: Create indexes on printer_configs (device_id, printer_name)
  - Migration 2: Create `app_settings` table (key PRIMARY KEY, value, value_type, description, updated_at)
  - Migration 2: Insert default app_settings (log_level, max_concurrent_downloads, max_concurrent_renders, default_batch_size, temp_file_retention_hours, auto_update_enabled, last_update_check)
**And** `shared/utils/unit_conversion.rs` must be created with functions:
- mm_to_pixels(mm: f64, dpi: u32) -> u32
- pixels_to_mm(pixels: u32, dpi: u32) -> f64
- mm_to_inches(mm: f64) -> f64
- inches_to_mm(inches: f64) -> f64
**And** database initialization must run at app startup in `main.rs`
**And** migrations must execute successfully on fresh database
**And** unit tests must verify:
- Connection opens successfully with WAL mode
- Migrations create correct schema
- Unit conversion functions are accurate (e.g., 210mm A4 width = 2480 pixels at 300 DPI)
**And** all tests pass with `cargo test`

### Story 2.2: Implement Windows Printer Discovery & Status Monitoring (Win32 API)

As a **nhân viên kho on Windows**,
I want **the app to automatically discover all connected printers and monitor their status**,
So that **I can see which printers are available and their current state**.

**Acceptance Criteria:**

**Given** the app is running on Windows
**When** I implement Windows printer infrastructure
**Then** the following must be created in `src-tauri/src/infrastructure/printer/windows/`:
- `win32_printer_manager.rs`: Implement PrinterManager trait using Win32 EnumPrinters API
  - `discover_printers() -> Vec<Printer>`: Returns all printers with name, device_id (from port), status, type
  - `get_status(name: &str) -> PrinterStatus`: Checks printer status (ONLINE/OFFLINE/ERROR)
  - Status updates every 5 seconds via polling
- `windows_printer_engine.rs`: Stub implementation of PrinterEngine trait (print method returns Ok for now)
**And** platform-specific compilation must use `#[cfg(target_os = "windows")]`
**And** Windows dependencies added to Cargo.toml: `windows = { version = "0.52", features = ["Win32_Graphics_Printing"] }`
**And** when no printers found, return empty Vec (not error)
**And** unit tests must verify:
- Printer discovery returns valid printer list structure
- Status mapping (Win32 status codes → PrinterStatus enum)
**And** integration test on Windows machine confirms real printer discovery works
**And** all tests pass with `cargo test`

### Story 2.3: Implement CUPS Printer Discovery & Status Monitoring (macOS/Linux)

As a **nhân viên kho on macOS or Linux**,
I want **the app to automatically discover all connected printers via CUPS and monitor their status**,
So that **I can see which printers are available regardless of my operating system**.

**Acceptance Criteria:**

**Given** the app is running on macOS or Linux with CUPS installed
**When** I implement CUPS printer infrastructure
**Then** the following must be created in `src-tauri/src/infrastructure/printer/cups/`:
- `cups_printer_manager.rs`: Implement PrinterManager trait
  - Primary: CUPS API via libcups bindings
  - Fallback 1: `lpstat` command parsing if CUPS API fails
  - Fallback 2: Parse `/etc/cups/printers.conf` if lpstat unavailable
  - `discover_printers() -> Vec<Printer>`: Returns all printers with name, device_id (from device-uri), status, type
  - `get_status(name: &str) -> PrinterStatus`: Checks via CUPS
  - Status updates every 5 seconds
- `cups_printer_engine.rs`: Stub implementation of PrinterEngine trait
**And** platform-specific compilation uses `#[cfg(not(target_os = "windows"))]`
**And** CUPS dependencies added: `cups-sys = "0.2"` (or similar CUPS bindings)
**And** fallback chain must be tested:
  - If CUPS API unavailable → try lpstat
  - If lpstat fails → try config file parsing
  - If all fail → return empty Vec with warning log
**And** unit tests verify:
- Fallback chain logic
- lpstat output parsing correctness
**And** manual test on macOS/Linux confirms real printer discovery
**And** all tests pass with `cargo test`

### Story 2.4: Implement SQLite Printer Repository & Config Persistence

As a **nhân viên kho**,
I want **my printer configurations to be saved and restored automatically**,
So that **I don't have to reconfigure settings every time I use the app**.

**Acceptance Criteria:**

**Given** the database schema and printer managers exist
**When** I implement the printer repository
**Then** `src-tauri/src/infrastructure/database/printer_repository.rs` must be created:
- Implement PrinterRepository trait from domain layer
- `save(printer: &Printer) -> Result<()>`: Insert or update printer_configs table
- `find_all() -> Result<Vec<Printer>>`: Load all printers from database
- `find_by_name(name: &str) -> Result<Option<Printer>>`: Query by printer_name
- Use prepared statements for all queries (SQL injection prevention)
- Convert units: Store margins in mm (from domain), display values use unit_conversion utilities
**And** `src-tauri/src/infrastructure/database/mod.rs` must export printer_repository
**And** AppContext (from Story 1.4) must be updated:
- Replace placeholder `printer_repo` with `Arc::new(SqlitePrinterRepository::new(db_pool))`
**And** unit tests with in-memory SQLite (`:memory:`) must verify:
- Save creates record with correct data
- Find operations return correct results
- Unit conversion (mm storage) works correctly
**And** integration test verifies data persists across app restarts
**And** all tests pass with `cargo test`

### Story 2.5: Create Printer Configuration UI - Basic Settings (3 Sections)

As a **nhân viên kho**,
I want **a clear configuration screen with basic printer settings organized into sections**,
So that **I can easily configure essential settings like printer selection, paper size, and margins**.

**Acceptance Criteria:**

**Given** the printer backend infrastructure is complete
**When** I create the basic printer configuration UI
**Then** the following React components must be created in `src/components/printer/`:

**PrinterConfigForm.tsx**: Main form component with 3 basic sections using @sapo/ui-components:
1. **Printer Selection**: Dropdown populated via `list_printers()` Tauri command
2. **Paper Settings**:
   - Paper Size dropdown (A4/A5/Letter/Custom)
   - Custom dimensions inputs (width × height mm) - shown only when Custom selected
   - Orientation checkbox (In chiều ngang)
3. **Layout**:
   - Margin inputs: Left, Right, Top, Bottom (mm)

**PrinterSelector.tsx**: Reusable printer dropdown component
**PrinterStatus.tsx**: Display printer status (ONLINE/OFFLINE/ERROR) with color indicator

**And** Tauri commands must be created in `src-tauri/src/interface/tauri/commands/printer.rs`:
- `list_printers() -> Result<Vec<PrinterDto>, AppError>`
- `save_printer_config(config: PrinterConfigDto) -> Result<(), AppError>`
- `get_printer_status(name: String) -> Result<PrinterStatusDto, AppError>`
**And** form validation using `react-hook-form` + `yup`:
- Paper size required
- Custom dimensions: 50-500mm range
- Margins: 0-100mm range
**And** UI must be in Vietnamese
**And** form must save config on submit and show success notification
**And** default printer must be auto-selected on first load (FR-2.4)
**And** manual test confirms all 3 sections render correctly and save works

### Story 2.6: Create Printer Configuration UI - Advanced Settings (2 Sections)

As a **nhân viên kho**,
I want **advanced printer settings in a separate section**,
So that **I can configure print mode and buffer settings when needed without cluttering the basic UI**.

**Acceptance Criteria:**

**Given** Story 2.5 (Basic Config UI) is complete
**When** I add advanced configuration sections
**Then** the PrinterConfigForm.tsx must be extended with 2 additional sections:

4. **Print Mode**:
   - "In ảnh" checkbox
   - Color mode dropdown (RGB/ARGB/BGR/GRAY/BINARY) - shown only when "In ảnh" checked
5. **Advanced**:
   - "Bật Printing Buffer" checkbox (default OFF)
   - Buffer Size input (KB) - shown only when buffer enabled

**And** form validation extended:
- Buffer size: 1-1024 KB if enabled
**And** save_printer_config command updated to handle all fields (basic + advanced)
**And** UI sections visually separated (e.g., "Cài đặt cơ bản" vs "Cài đặt nâng cao" headers)
**And** manual test confirms advanced settings save correctly and conditional fields show/hide properly

### Story 2.6: Implement Secret Management for Device Tokens (OS Keychain)

As a **developer**,
I want **device tokens and HMAC signing keys stored securely in OS keychain/credential manager**,
So that **secrets are encrypted by the OS and not leaked if SQLite database is compromised**.

**Acceptance Criteria:**

**Given** the application needs to store sensitive device tokens
**When** I implement secret management
**Then** the following must be created in `src-tauri/src/infrastructure/secrets/`:
- `secret_manager.rs`: Define SecretManager trait:
  - `store(key: &str, value: &str) -> Result<()>`
  - `retrieve(key: &str) -> Result<Option<String>>`
  - `delete(key: &str) -> Result<()>`

**Platform-specific implementations**:
- `windows_credential_manager.rs` (`#[cfg(target_os = "windows")]`):
  - Use Windows Credential Manager API via `windows` crate
- `macos_keychain.rs` (`#[cfg(target_os = "macos")]`):
  - Use Keychain Services via `security-framework` crate
- `linux_secret_service.rs` (`#[cfg(target_os = "linux")]`):
  - Use Secret Service API via `secret-service` crate

**And** dependencies added to Cargo.toml:
- `windows = { version = "0.52", features = ["Security_Credentials"] }`
- `security-framework = "2.9"` (macOS)
- `secret-service = "3.0"` (Linux)
**And** AppContext updated with `secret_manager: Arc<dyn SecretManager>`
**And** secrets stored with app-specific namespace: "com.sapo.printer"
**And** unit tests verify:
- Store/retrieve round-trip works
- Delete removes secret
- Retrieve non-existent key returns None (not error)
**And** platform-specific integration tests confirm real OS keychain storage
**And** all tests pass with `cargo test` on respective platforms

---

## Epic 3: Bulk Print Job Management with Document Processing Pipeline

Nhân viên kho có thể in hàng loạt phiếu (1-5000 đơn) với một click. App tự động download PDFs từ S3, render với hybrid strategy (Direct PDF hoặc MuPDF), apply margins/color modes, batch processing, auto-retry khi lỗi, cancel jobs, cleanup temp files tự động, và hiển thị real-time status dashboard.

**FRs covered:** FR-1.1, FR-1.2, FR-1.3, FR-1.4, FR-1.5, FR-3.1, FR-3.2, FR-3.3, FR-3.4, FR-3.5, FR-4.1, FR-4.4, AR-5 (Strategy Pattern), AR-6 (Queue Management + Circuit Breaker), AR-7 (RAII Cleanup), AR-11 (Tauri Integration), AR-14 (Error Handling), NFR-1 (Performance), NFR-2 (Reliability)

**Implementation Notes:** Document processing pipeline (download, hybrid rendering, color conversion, temp file management) + Print job aggregate + Durable queue + Auto-retry logic + Real-time UI updates + Event handlers

**Dependencies:** Epic 1 (Foundation), Epic 2 (Printer Infrastructure)

---

## Epic 3: Bulk Print Job Management with Document Processing Pipeline

### Story 3.1: Implement S3 Document Downloader with Circuit Breaker

### Story 3.1: Implement S3 Document Downloader with Circuit Breaker

As a **developer**,
I want **to implement a resilient document downloader with circuit breaker for S3 URLs**,
So that **the app can download PDFs reliably and fail gracefully when S3 is unavailable**.

**Acceptance Criteria:**

**Given** the infrastructure layer exists
**When** I implement the document downloader
**Then** the following must be created in `src-tauri/src/infrastructure/downloader/`:
- `document_downloader.rs`: Define DocumentDownloader trait:
  - `download(url: Url, job_id: JobId) -> Result<PathBuf>`
- `reqwest_downloader.rs`: Implement DocumentDownloader using reqwest:
  - Download PDF to temp path: `~/.sapo-printer/temp/{job_id}.tmp`
  - Validate PDF header after download (starts with `%PDF`)
  - Atomic rename: `.tmp` → `.pdf` only after validation succeeds
  - 30 second timeout per FR-3.1
  - Return final `.pdf` PathBuf
- `circuit_breaker.rs`: Circuit breaker implementation:
  - States: Closed, Open, HalfOpen
  - Threshold: 5 failures → Open
  - Timeout: 60s in Open state before HalfOpen
  - Wrap download calls in circuit breaker
  - Return CircuitOpenError immediately when Open
**And** dependencies added to Cargo.toml: `reqwest = { version = "0.11", features = ["blocking"] }`
**And** unit tests verify:
  - Valid PDF download succeeds and returns `.pdf` path
  - Invalid file (non-PDF) fails validation, `.tmp` cleaned up
  - Timeout after 30s
  - Circuit breaker opens after 5 consecutive failures
  - Circuit breaker transitions: Closed → Open → HalfOpen → Closed
**And** integration test downloads real PDF from mock S3 endpoint
**And** all tests pass with `cargo test`

### Story 3.2: Implement MuPDF Renderer with Color Mode Support

As a **nhân viên kho**,
I want **PDFs rendered with specific color modes and margins applied**,
So that **printed output matches my printer capabilities and layout requirements**.

**Acceptance Criteria:**

**Given** the document downloader exists
**When** I implement the MuPDF renderer
**Then** the following must be created in `src-tauri/src/infrastructure/renderer/`:
- `document_renderer.rs`: Define DocumentRenderer trait:
  - `render(path: &Path, config: RenderConfig) -> Result<Vec<u8>>`
- `pdfium_renderer.rs`: Implement DocumentRenderer using MuPDF bindings:
  - Load PDF from path
  - Apply margins: Convert mm → pixels at 300 DPI using `unit_conversion::mm_to_pixels()`
  - Apply color mode conversion (RGB/ARGB/BGR/GRAY/BINARY per FR-3.3)
  - Render pages to bitmap at 300 DPI
  - Return raw bitmap bytes
- `RenderConfig` struct:
  - Fields: margin_left, margin_right, margin_top, margin_bottom (all in mm)
  - color_mode: ColorMode enum (RGB, ARGB, BGR, GRAY, BINARY)
  - paper_size: PaperSize struct (width, height in mm)
**And** dependencies added: `mupdf-sys = "0.3"` (or similar MuPDF bindings)
**And** color mode conversions must be accurate:
  - RGB: 24-bit (8 bits per channel)
  - ARGB: 32-bit with alpha channel
  - BGR: Windows default byte order
  - GRAY: 8-bit grayscale
  - BINARY: 1-bit monochrome
**And** unit tests verify:
  - Margin conversion correctness (10mm → 118 pixels at 300 DPI)
  - Color mode outputs correct bit depth
  - A4 page (210×297mm) renders to correct pixel dimensions (2480×3508 at 300 DPI)
**And** integration test renders sample PDF with all 5 color modes
**And** all tests pass with `cargo test`

### Story 3.3: Implement Direct PDF Strategy

As a **nhân viên kho**,
I want **fast Direct PDF printing when the printer supports it**,
So that **jobs complete in ~0.5s instead of ~2-3s render time**.

**Acceptance Criteria:**

**Given** the MuPDF renderer exists
**When** I implement Direct PDF strategy
**Then** `src-tauri/src/infrastructure/renderer/direct_pdf_renderer.rs` must be created:
- Implement DocumentRenderer trait
- `render(path: &Path, config: RenderConfig) -> Result<Vec<u8>>`:
  - Read PDF file as raw bytes (no rendering)
  - Return PDF bytes directly (pass-through)
  - Ignore margins/color mode config (printer handles natively)
  - Execution time target: ~0.5s per FR-3.2
**And** unit tests verify:
  - Direct PDF returns raw PDF bytes unchanged
  - Execution time < 1s for typical invoice PDF (~100KB)
  - No MuPDF dependency loaded (pure file read)
**And** integration test confirms:
  - Output is valid PDF (header `%PDF`)
  - File size matches input (no rendering overhead)
**And** all tests pass with `cargo test`

### Story 3.4: Create Hybrid Strategy Selector (Auto-detect printer capability)

As a **developer**,
I want **automatic strategy selection based on printer capabilities**,
So that **the app uses fast Direct PDF when possible, falls back to MuPDF render when needed**.

**Acceptance Criteria:**

**Given** both Direct PDF and MuPDF renderers exist
**When** I implement the hybrid strategy selector
**Then** `src-tauri/src/infrastructure/renderer/strategy_selector.rs` must be created:
- `select_strategy(printer: &Printer) -> Box<dyn DocumentRenderer>`
- Detection logic:
  - Check if printer supports native PDF (via printer capabilities)
  - If YES and margins == 0 and color_mode == RGB → DirectPdfRenderer
  - Otherwise → PdfiumRenderer (MuPDF)
- Cache strategy decision per printer (5s TTL, aligned with printer status polling)
**And** `PrinterManager` trait extended with:
  - `supports_direct_pdf(printer_name: &str) -> bool`
  - Windows: Check via Win32 GetPrinterDriver capabilities
  - CUPS: Check via `lpoptions -d {printer} -l` for "pdftopdf" support
**And** AppContext updated with:
  - `renderer: Arc<dyn DocumentRenderer>` field
  - Strategy selected at job creation time based on printer
**And** unit tests verify:
  - Printer with PDF support + no margins → Direct PDF selected
  - Printer with PDF support + margins > 0 → MuPDF selected
  - Printer without PDF support → MuPDF selected
  - Strategy cache expires after 5s
**And** integration test confirms:
  - Strategy switches when printer changes
  - Performance: Direct PDF jobs complete in < 1s, MuPDF jobs in 2-3s
**And** all tests pass with `cargo test`

### Story 3.5: Implement RAII Temp File Management with Tiered Cleanup

As a **developer**,
I want **automatic temp file cleanup with RAII pattern and tiered retention**,
So that **disk space is reclaimed immediately on success, but failed jobs kept for debugging**.

**Acceptance Criteria:**

**Given** the document downloader creates temp files
**When** I implement temp file management
**Then** `src-tauri/src/infrastructure/temp_file.rs` must be created with TempPdfFile struct using RAII pattern (Drop trait auto-deletes when keep_on_drop is false)
**And** cleanup strategies implemented:
1. **Immediate** (Success): Delete in Drop when job COMPLETED
2. **Deferred** (Failure): Keep for 24h when job FAILED (set keep_on_drop = true)
3. **Startup**: On app launch, delete all .tmp files and .pdf files older than 24h in ~/.sapo-printer/temp/
**And** `startup_cleanup()` function in `main.rs` scans temp directory and removes old files
**And** unit tests verify:
  - TempPdfFile auto-deletes on drop when keep_on_drop = false
  - TempPdfFile kept on drop when keep_on_drop = true
  - Startup cleanup removes old files (>24h)
  - Startup cleanup preserves recent files (<24h)
**And** integration test verifies file lifecycle across drop and app restart
**And** all tests pass with `cargo test`

---

### Story 3.6: Create Print Job Tables & Event Store Schema

As a **developer**,
I want **to create database schemas for print jobs and event store**,
So that **jobs can be persisted with full audit trail and the system supports event sourcing**.

**Acceptance Criteria:**

**Given** the database migration system exists (from Story 2.1)
**When** I create print job schemas
**Then** Migration 3 must create print_jobs table with columns: id (TEXT PRIMARY KEY UUID), printer_name, document_url, status, retry_count (INTEGER DEFAULT 0), created_at, updated_at, completed_at (NULL), with indexes on status and created_at
**And** Migration 4 must create events table with columns: id (INTEGER PRIMARY KEY AUTOINCREMENT), aggregate_id, sequence_number, event_type, payload (JSON), timestamp, hmac, with UNIQUE constraint on (aggregate_id, sequence_number) and indexes on aggregate_id and event_type
**And** unit tests verify migrations create correct schemas and constraints work
**And** integration test verifies fresh database runs all migrations and can insert/query both tables
**And** all tests pass with cargo test

### Story 3.7: Implement SQLite Print Job Repository

As a **developer**,
I want **to implement the PrintJobRepository trait with SQLite**,
So that **print jobs can be persisted and retrieved efficiently**.

**Acceptance Criteria:**

**Given** the print_jobs table exists
**When** I implement the repository
**Then** print_job_repository.rs must implement PrintJobRepository trait with save, update, find_by_id, find_by_status methods using prepared statements
**And** event_store.rs must implement save_event, save_all (batch insert in transaction), find_by_aggregate methods with JSON serialization
**And** AppContext updated with real SqlitePrintJobRepository
**And** unit tests with in-memory SQLite verify all CRUD operations and event sequencing
**And** all tests pass with cargo test

### Story 3.3: Implement CreatePrintJobUseCase with Event Publishing

As a **nhân viên kho**,
I want **to create a print job via Tauri command**,
So that **I can initiate a print request from the UI**.

**Acceptance Criteria:**

**Given** PrintJobRepository and EventBus exist
**When** I implement CreatePrintJobUseCase
**Then** use case must follow Outbox Pattern: validate request (1-5000 URLs), load printer (verify ONLINE), create PrintJob aggregate, collect events, save job and events in transaction, publish events after commit, return job_id
**And** CreateJobRequest DTO and create_print_job Tauri command created
**And** unit tests verify valid request succeeds, invalid request (>5000 URLs or offline printer) returns ApplicationError, events published only after commit, transaction rollback on failure
**And** integration test confirms job persists in database
**And** all tests pass with cargo test

### Story 3.4: Implement Durable Queue Manager (SQLite-backed)

As a **developer**,
I want **a durable queue backed by SQLite**,
So that **jobs persist across app restarts and aren't lost on crash**.

**Acceptance Criteria:**

**Given** the print_jobs table exists
**When** I implement queue manager
**Then** QueueManager must implement push (update status to QUEUED), pop (SELECT oldest QUEUED, update to PROCESSING), requeue (update with delay) methods backed by print_jobs table
**And** PushToQueueHandler event handler listens for PrintJobCreated and calls queue_manager.push
**And** AppContext updated with queue_manager
**And** unit tests verify push/pop FIFO order, pop returns None when empty, requeue sets future time
**And** integration test verifies jobs persist across simulated restart
**And** all tests pass with cargo test

### Story 3.5: Implement Queue Worker with Batch Processing

As a **developer**,
I want **a queue worker that processes jobs sequentially**,
So that **print jobs are executed automatically from the queue**.

**Acceptance Criteria:**

**Given** queue manager and document pipeline exist
**When** I implement queue worker
**Then** QueueWorker must process jobs in loop: pop from queue, download (update DOWNLOADED + publish event), render (update SUBMITTED + publish event), print (update PRINTING then COMPLETED + publish event), temp file auto-deleted via Drop
**And** worker starts automatically in main.rs, runs in background thread with start/stop methods
**And** concurrency limit: max 1 worker thread (sequential processing)
**And** unit tests verify worker processes through all states and publishes events
**And** integration test verifies real job flow QUEUED → COMPLETED with temp cleanup
**And** all tests pass with cargo test

### Story 3.6: Implement Auto-Retry Logic with Exponential Backoff

As a **nhân viên kho**,
I want **failed jobs to automatically retry up to 3 times**,
So that **transient errors don't require manual intervention**.

**Acceptance Criteria:**

**Given** queue worker exists
**When** I implement auto-retry
**Then** error handling added to worker: retryable errors (timeout, render error, printer temporary) requeue with exponential backoff (5s, 10s, 20s), non-retryable errors (validation, 404) fail immediately, max 3 retries enforced by domain rule
**And** PrintJob aggregate updated with increment_retry and mark_failed methods
**And** is_retryable helper function distinguishes error types
**And** unit tests verify retry logic: retryable → requeue with correct delay, retry_count increments, after 3 retries → stays FAILED, non-retryable → immediate FAILED
**And** integration test verifies transient error recovers on retry with correct timing
**And** all tests pass with cargo test

### Story 3.7: Implement Job Cancellation Use Case

As a **nhân viên kho**,
I want **to cancel a queued or in-progress print job**,
So that **I can stop jobs I no longer need**.

**Acceptance Criteria:**

**Given** CreatePrintJobUseCase exists
**When** I implement CancelPrintJobUseCase
**Then** use case must load job, validate cancellable (domain rule: not COMPLETED or FAILED), call job.cancel, update repository, cleanup temp file
**And** PrintJob aggregate updated with cancel method: validates status, updates to CANCELLED, publishes PrintJobCancelled event
**And** cancel_print_job Tauri command created
**And** unit tests verify can cancel PENDING/QUEUED/DOWNLOADED, cannot cancel COMPLETED/FAILED (DomainError), PRINTING cancellation succeeds with warning
**And** integration test verifies QUEUED job cancelled and temp file deleted
**And** all tests pass with cargo test

### Story 3.8: Create Print Job List UI with Filters

As a **nhân viên kho**,
I want **a dashboard showing all my print jobs in a table with filtering capability**,
So that **I can see job status, search, and filter by different criteria**.

**Acceptance Criteria:**

**Given** backend job management complete
**When** I create print job list UI
**Then** React components created: 
- PrintJobDashboard (main container with filters)
- PrintJobTable (table with columns: ID, Printer, Status, Progress, Created, Actions)
- PrintJobCard (status badge with colors, action buttons)
- PrintJobFilters (filter by status, date range, printer)
**And** Tauri commands created:
- `get_job_status(job_id: String) -> Result<JobStatusDto, AppError>`
- `list_jobs(filter: JobFilterDto) -> Result<Vec<JobDto>, AppError>`
**And** UI in Vietnamese: 
- Status labels (Đang chờ, Đang tải, Đang in, Hoàn thành, Thất bại)
- Column headers (Mã job, Máy in, Trạng thái, Tiến trình, Thời gian tạo, Hành động)
- Action buttons (Hủy, Thử lại)
- Filter labels (Lọc theo trạng thái, Từ ngày, Đến ngày, Máy in)
**And** manual test confirms:
- Table displays job list correctly
- Filters work (status, date range, printer)
- Action buttons (Cancel, Retry) are enabled/disabled correctly
- Pagination works for large job lists

### Story 3.9: Add Real-time Status Updates to Dashboard

As a **nhân viên kho**,
I want **real-time updates on the dashboard without manual refresh**,
So that **I can see progress automatically as jobs are processed**.

**Acceptance Criteria:**

**Given** Story 3.8 (Print Job List UI) is complete
**When** I add real-time updates
**Then** Tauri event subscriptions implemented:
- `job_status_changed` → update job row status badge
- `download_progress` → update progress bar
- `print_progress` → update progress bar
**And** Backend emits events:
- After each state transition (PENDING → QUEUED → DOWNLOADED → PRINTING → COMPLETED)
- During download every 10% progress
- During print every 10% progress
**And** UI components updated:
- PrintJobCard shows animated progress bar (0-100%)
- Progress bar color changes by status (blue=downloading, green=printing, red=failed)
- Status badge updates automatically without page refresh
- Time elapsed counter updates every second for in-progress jobs
**And** Event handling optimizations:
- Debounce rapid updates (max 1 update per 200ms per job)
- Only update visible rows (virtual scrolling if >100 jobs)
**And** manual test confirms:
- Dashboard updates automatically as jobs progress
- Progress bar animates smoothly
- Multiple jobs update independently
- No performance issues with 50+ concurrent jobs

---

## Epic 4: System Integration & Production Readiness

Desktop app tích hợp với web app qua Native Messaging, tự động update khi có version mới, có audit trail đầy đủ, metrics collection, production-ready deployment.

### Story 4.1: Implement Native Messaging Protocol (JSON commands)

As a **web app developer**,
I want **the desktop app to communicate with the browser via Native Messaging**,
So that **users can trigger print jobs directly from the web interface**.

**Acceptance Criteria:**

**Given** the Tauri app is installed
**When** I implement Native Messaging
**Then** protocol.rs must implement JSON command handlers: ping, print_batch (calls CreatePrintJobUseCase), get_status, cancel_job, list_printers
**And** registry.rs implements browser registration for Windows (Registry), macOS/Linux (JSON manifest in NativeMessagingHosts)
**And** Native Messaging manifest created with allowed_origins for security
**And** origin validation rejects unauthorized sources
**And** error responses follow standard format
**And** unit tests verify all commands parse correctly, invalid origin rejected, malformed JSON returns error
**And** integration test with mock Chrome extension confirms bidirectional communication
**And** all tests pass with cargo test

### Story 4.2: Implement Status Polling Sync (2s interval)

As a **web app**,
I want **to poll the desktop app every 2 seconds for job status updates**,
So that **users see near real-time progress in the browser**.

**Acceptance Criteria:**

**Given** Native Messaging implemented
**When** I implement status polling
**Then** GetJobStatusUseCase created returning JobStatusDto with job_id, status, progress (0-100), printer_name, timestamps, error_message
**And** get_status Native Messaging command calls use case
**And** Web app polling documentation provided (poll every 2s, stop on COMPLETED/FAILED)
**And** unit tests verify use case returns correct DTO, JobNotFound for non-existent job, progress calculation per state (PENDING=0%, QUEUED=10%, DOWNLOADED=40%, SUBMITTED=60%, PRINTING=80%, COMPLETED=100%)
**And** integration test verifies polling multiple jobs
**And** all tests pass with cargo test

### Story 4.3: Setup Structured Logging with Tracing Crate

As a **developer**,
I want **structured logging throughout the application**,
So that **I can debug issues efficiently with searchable, filterable logs**.

**Acceptance Criteria:**

**Given** shared layer exists
**When** I setup logging
**Then** tracing_setup.rs must initialize tracing_subscriber with JSON formatter, log level (Development=DEBUG, Production=INFO via RUST_LOG), log file at ~/.sapo-printer/logs/app.log, daily rotation with 7-day retention
**And** logging initialized in main.rs before other operations
**And** logging added throughout: Use Cases (entry/exit), Repositories (queries), Infrastructure (external calls), Error paths (full context)
**And** dependencies added: tracing, tracing-subscriber with json and env-filter features, tracing-appender
**And** unit tests verify logger initializes, log levels filter correctly, log rotation creates dated files
**And** integration test verifies logs written during operations, old files deleted after 7 days
**And** all tests pass with cargo test

### Story 4.4: Implement Event Store Audit Trail with HMAC Signing (30 days retention)

As a **system administrator**,
I want **tamper-proof audit logs of all print jobs**,
So that **I can verify job history for compliance and troubleshooting**.

**Acceptance Criteria:**

**Given** event store exists
**When** I implement audit trail
**Then** HMAC-SHA256 signing added to event store: HMAC(secret_key, aggregate_id + sequence_number + event_type + payload + timestamp), signing key from SecretManager
**And** audit.rs created with verify_event_integrity, get_audit_trail, cleanup_old_events (>30 days) functions
**And** startup cleanup in main.rs
**And** AuditTrailUseCase returns chronological event list
**And** get_job_audit_trail Tauri command created
**And** unit tests verify HMAC generation deterministic, verification detects tampering, audit trail correct order, cleanup deletes old events
**And** integration test verifies real events with valid HMAC, tampered event fails verification
**And** all tests pass with cargo test

### Story 4.5: Implement Metrics Collection & Export

As a **system administrator**,
I want **to view operational metrics about print jobs**,
So that **I can monitor system health and optimize performance**.

**Acceptance Criteria:**

**Given** application running
**When** I implement metrics collection
**Then** collector.rs created with MetricsCollector singleton tracking: job metrics (total, by status, success rate), queue metrics (size, wait time), printer metrics (jobs per printer, utilization), performance metrics (duration, P50/P95/P99 latencies, download/render time)
**And** metrics integrated: queue worker records duration and size, use cases increment counters, repositories track latencies
**And** GetMetricsUseCase returns MetricsDto snapshot
**And** get_metrics Tauri command created
**And** optional MetricsPanel UI component displays key metrics
**And** unit tests verify metrics increment correctly, percentiles accurate, success rate computed
**And** integration test verifies metrics update during real job execution
**And** all tests pass with cargo test

### Story 4.6: Setup Tauri Auto-Update with Code Signing

As a **user**,
I want **the app to automatically check for updates and install them securely**,
So that **I always have the latest features and bug fixes**.

**Acceptance Criteria:**

**Given** Tauri app deployed
**When** I setup auto-update
**Then** tauri.conf.json configured with updater plugin: endpoints pointing to GitHub releases, dialog enabled, pubkey for signature verification
**And** latest.json manifest generated during release with version, notes, platforms (Windows/macOS/Linux with signatures and URLs)
**And** code signing setup documented: Windows (EV cert + signtool), macOS (Apple Developer ID + codesign + notarization), Linux (GPG signing)
**And** updater/mod.rs created with check_for_updates and install_update functions
**And** auto-update check on startup and every 24h
**And** dependency added: tauri-plugin-updater
**And** unit tests verify update check parses manifest, version comparison works
**And** manual test verifies app checks on launch, update download/installation works, signature verification prevents tampering
**And** build script documentation includes signing steps

### Story 4.7: Create Auto-Update Popup UI

As a **user**,
I want **a clear notification when an update is available**,
So that **I can decide whether to install it now or later**.

**Acceptance Criteria:**

**Given** auto-update check implemented
**When** I create update UI
**Then** UpdatePopup.tsx created with app icon, current/available versions, release notes (first 3 bullets), "Có gì mới" link, action buttons: "Cập nhật ngay" (primary, shows progress, restarts app), "Đóng" (dismiss), "Kiểm tra phiên bản" (manual check)
**And** popup triggers on startup if update available and on manual check
**And** Tauri commands created: check_updates, install_update
**And** UI in Vietnamese
**And** popup as modal overlay (non-blocking)
**And** manual test confirms popup appears correctly, installation works and restarts, buttons function correctly
