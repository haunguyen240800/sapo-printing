# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**sapo-printer** is a desktop printing application for bulk print job management in the SAPO retail management system. The client receives print commands from a web application, manages print jobs, downloads documents, renders them, and sends them to printers with full audit trails and retry capabilities.

This is a greenfield project following **Domain-Driven Design (DDD)** and **Clean Architecture** principles, documented in Vietnamese.

## Architecture

The system follows a strict **4-layer Clean Architecture**:

```
Interface Layer (REST, WebSocket, Tauri)
    ↓
Application Layer (Use Cases, Handlers, Services)
    ↓
Domain Layer (Aggregates, Events, Repository Contracts)
    ↑
Infrastructure Layer (SQLite, PDFium, Windows API, Reqwest, EventBus, Queue)
```

### Dependency Rules (Critical)

1. **Domain Layer** is completely independent — no dependencies on any other layer
2. **Application Layer** only depends on Domain
3. **Infrastructure Layer** implements Domain contracts (repositories, services)
4. **Interface Layer** calls Application Use Cases only
5. Never access database directly from controllers
6. Never print directly from REST API — all print requests must create a PrintJob
7. All state changes must publish Domain Events
8. All jobs must go through the Queue

## Bounded Contexts

The system is organized into distinct contexts:

- **Printer Management** — Printer aggregate, status tracking
- **Print Job Management** — PrintJob aggregate with PrintTask entities
- **Document Management** — Document aggregate, download, and rendering
- **Queue Management** — Job queuing and worker processing (Infrastructure concern, not Domain)
- **Configuration Management** — App, printer, and queue configuration
- **Monitoring Management** — Metrics, logging, and audit trails

## Domain Model

### PrintJob Aggregate

**Aggregate Root:** `PrintJob`

**Entities:** `PrintTask`

**Value Objects:** `JobId`, `PrinterName`, `PrintStatus`, `PaperSize`

**Domain Events:**
- `PrintJobCreated` → triggers `PushToQueueHandler`
- `PrintJobQueued`
- `PrintJobStarted`
- `PrintJobDownloaded`
- `PrintJobRendered`
- `PrintJobSubmitted`
- `PrintJobCompleted` → triggers `UpdateHistoryHandler`
- `PrintJobFailed`

**Business Rules:**
- Cannot print a job that is already `COMPLETED`
- Cannot retry more than `MAX_RETRY` times
- Cannot cancel a job that is already `COMPLETED`

### Printer Aggregate

**Aggregate Root:** `Printer`

**Value Objects:** `PrinterId`, `PrinterName`, `PrinterType`, `PrinterStatus`

**Domain Events:**
- `PrinterConnected`
- `PrinterDisconnected`

**Business Rules:**
- Only `ONLINE` printers can receive jobs

### Document Aggregate

**Aggregate Root:** `Document`

**Value Objects:** `DocumentId`, `DocumentType`, `DocumentLocation`

**Document Types:** `PDF`, `IMAGE`, `ZPL`, `RAW`

## Key Use Cases

### CreatePrintJobUseCase
1. Validate Request
2. Load Printer
3. Create Document
4. Create PrintJob
5. Persist
6. Publish `PrintJobCreated` Event

**Output:** `JobId`

### RetryPrintJobUseCase
1. Load Job
2. Validate Retry (max 3 attempts)
3. Update Status
4. Push to Queue

### CancelPrintJobUseCase
1. Load Job
2. Validate Status
3. Cancel

### ListPrinterUseCase
1. Load Printers
2. Return DTO

## Expected Project Structure

```
src-tauri/
├── interface/          # REST, WebSocket, Tauri handlers
├── application/        # DTOs, Use Cases, Event Handlers, Services
├── domain/
│   ├── print_job/      # PrintJob aggregate, events, repository traits
│   ├── printer/        # Printer aggregate, events, repository traits
│   └── document/       # Document aggregate, value objects
├── infrastructure/
│   ├── database/       # SQLite repositories
│   ├── queue/          # QueueManager, QueueWorker
│   ├── downloader/     # ReqwestDocumentDownloader
│   ├── renderer/       # PdfiumRenderer, ImageRenderer, ZplRenderer
│   ├── printer/        # WindowsPrinterEngine
│   └── eventbus/       # Event publishing infrastructure
├── shared/
│   ├── errors/         # ApplicationError, DomainError, InfrastructureError
│   ├── logger/         # Tracing setup
│   └── config/         # AppConfig, PrinterConfig, QueueConfig
└── main.rs
```

## REST API Endpoints (Interface Layer)

```
POST   /print-jobs              # Create print job
GET    /print-jobs/{id}         # Get job status
POST   /print-jobs/{id}/retry   # Retry failed job
POST   /print-jobs/{id}/cancel  # Cancel job
GET    /printers                # List available printers
```

Controllers must:
- Validate DTOs
- Call Use Cases
- Return DTOs
- **Never** contain business logic

## Queue Processing

**QueueWorker Flow:**
1. Receive Job from Queue
2. Download file via `DocumentDownloader`
3. Render via `DocumentRenderer`
4. Send to Printer via `PrinterEngine`
5. Update Status and publish events
6. Auto-retry on failure (max 3 times)

**Batch Processing:**
- Default batch size: 50 orders
- Maximum: 5000 orders per bulk print request
- Auto-split into batches by server

## Domain Contracts (Traits)

### Repository Traits
```rust
trait PrintJobRepository {
    fn save();
    fn update();
    fn find_by_id();
}

trait PrinterRepository {
    fn save();
    fn find_all();
    fn find_by_name();
}
```

### Service Traits
```rust
trait DocumentDownloader { ... }  // Impl: ReqwestDocumentDownloader
trait DocumentRenderer { ... }    // Impl: PdfiumRenderer, ImageRenderer, ZplRenderer
trait PrinterManager { ... }      // List, get default, check status
trait PrinterEngine { ... }       // Impl: WindowsPrinterEngine
```

## Cross-Cutting Concerns

**Logging:** Use `tracing` crate for structured logging

**Metrics to track:**
- Job count by status
- Queue size
- Print duration per job

**Error Types:**
- `ApplicationError` — Use Case validation errors
- `DomainError` — Business rule violations
- `InfrastructureError` — External system failures

## Business Rules & Constraints

1. Maximum 5000 orders per bulk print request
2. Auto-select "Bulk Print" mode when ≥100 orders
3. Batch size defaults to 50 orders
4. Maximum 3 retry attempts per failed job
5. Auto-retry failed print jobs
6. Real-time status updates via WebSocket
7. All jobs require valid shipping labels before printing

## BMad Framework

This project uses the **BMad Method** framework for software development workflows located in `_bmad/`.

**Key Modules:**
- **BMM** (BMad Method Module) — Planning and implementation artifacts
- **TEA** (Test Engineering & Automation) — Test design, execution, and traceability
- **CIS** (Creative & Innovation Studio) — Design thinking, brainstorming, storytelling
- **BMB** (BMad Builder) — Agent and skill construction

**Output Locations:**
- Planning artifacts: `_bmad-output/planning-artifacts/`
- Implementation artifacts: `_bmad-output/implementation-artifacts/`
- Test artifacts: `_bmad-output/test-artifacts/`

**Documentation Language:** Vietnamese (as configured in `_bmad/config.toml`)

**Project Knowledge Base:** `docs/` directory contains:
- `srs-in.md` — Architecture specification (DDD + Clean Architecture)
- `SRS_In số lượng lớn.md` — Detailed SRS for bulk printing feature (387KB)

## Development Notes

- This is a **Windows desktop application** using Tauri
- Printer integration via **Windows Print System API**
- Document rendering via **PDFium** library
- HTTP client: **Reqwest**
- Database: **SQLite**
- Real-time communication: **WebSocket**
- Language: **Rust** (expected based on architecture doc)

## Architectural Principles (Non-Negotiable)

1. Domain layer must remain completely independent
2. Business rules exist **only** in the Domain layer
3. Use Cases exist **only** in the Application layer
4. Infrastructure only implements contracts — no business logic
5. All state transitions must emit Domain Events
6. Queue is an Infrastructure concern, not Domain
7. Infrastructure can be replaced without affecting Domain (Dependency Inversion Principle)
8. System follows Event-Driven Architecture throughout
