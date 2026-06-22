# Printer Client Architecture

## Domain-Driven Design + Clean Architecture

Version: 1.0

---

# 1. Mục tiêu hệ thống

Printer Client là một Desktop Application chạy trên máy người dùng.

Trách nhiệm:

* Nhận lệnh in từ Web Application
* Quản lý Print Job
* Quản lý Printer
* Download tài liệu
* Render tài liệu
* Gửi dữ liệu tới Printer
* Theo dõi trạng thái in
* Retry khi lỗi
* Audit toàn bộ quá trình

---

# 2. Kiến trúc tổng thể

```text
                     +----------------+
                     |   Web System   |
                     +--------+-------+
                              |
                              |
                              | REST
                              |
                              v

+----------------------------------------------------------+
|                    Printer Client                        |
|                                                          |
| +------------------------------------------------------+ |
| |                  Interface Layer                     | |
| +------------------------------------------------------+ |
|                           |                              |
|                           v                              |
| +------------------------------------------------------+ |
| |                 Application Layer                    | |
| +------------------------------------------------------+ |
|                           |                              |
|                           v                              |
| +------------------------------------------------------+ |
| |                    Domain Layer                      | |
| +------------------------------------------------------+ |
|                           |                              |
|                           v                              |
| +------------------------------------------------------+ |
| |                Infrastructure Layer                  | |
| +------------------------------------------------------+ |
+----------------------------------------------------------+
                           |
                           v
                 Windows Print System
                           |
                           v
                        Printer
```

---

# 3. Dependency Rule

```text
Interface
    ↓
Application
    ↓
Domain

Infrastructure
    ↑
```

Nguyên tắc:

* Domain không phụ thuộc bất kỳ layer nào
* Application chỉ phụ thuộc Domain
* Infrastructure triển khai Domain Contract
* Interface gọi Application

---

# 4. Bounded Context

Hệ thống được chia thành các Context sau:

```text
Printer Management

Print Job Management

Document Management

Queue Management

Configuration Management

Monitoring Management
```

---

# 5. Context Map

```text
+--------------------+
| Printer Management |
+---------+----------+
          |
          |
          v
+--------------------+
| Print Job          |
+---------+----------+
          |
          |
          v
+--------------------+
| Document           |
+---------+----------+
          |
          |
          v
+--------------------+
| Queue              |
+---------+----------+
          |
          |
          v
+--------------------+
| Monitoring         |
+--------------------+
```

---

# 6. Domain Layer

## 6.1 Aggregate

### PrintJob Aggregate

Aggregate Root

```text
PrintJob
```

Entity

```text
PrintTask
```

Value Object

```text
JobId
PrinterName
PrintStatus
PaperSize
```

Domain Event

```text
PrintJobCreated
PrintJobQueued
PrintJobStarted
PrintJobCompleted
PrintJobFailed
```

---

### Printer Aggregate

Aggregate Root

```text
Printer
```

Value Object

```text
PrinterId
PrinterName
PrinterType
PrinterStatus
```

Domain Event

```text
PrinterConnected
PrinterDisconnected
```

---

### Document Aggregate

Aggregate Root

```text
Document
```

Value Object

```text
DocumentId
DocumentType
DocumentLocation
```

DocumentType

```text
PDF
IMAGE
ZPL
RAW
```

---

# 7. Domain Model

## PrintJob

```rust
pub struct PrintJob {
    id: JobId,
    printer_name: PrinterName,
    document_id: DocumentId,
    status: PrintStatus,
    retry_count: u8,
}
```

Business Rules

```text
Không được in Job đã COMPLETED

Không được Retry quá MAX_RETRY

Không được Cancel Job đã COMPLETED
```

---

## Printer

```rust
pub struct Printer {
    id: PrinterId,
    name: PrinterName,
    status: PrinterStatus,
}
```

Business Rules

```text
Chỉ Printer ONLINE mới được nhận Job
```

---

# 8. Domain Events

```text
PrintJobCreated

PrintJobQueued

PrintJobStarted

PrintJobDownloaded

PrintJobRendered

PrintJobSubmitted

PrintJobCompleted

PrintJobFailed
```

Mọi thay đổi trạng thái phải thông qua Domain Event.

---

# 9. Repository Contracts

## PrintJobRepository

```rust
trait PrintJobRepository {
    fn save();
    fn update();
    fn find_by_id();
}
```

---

## PrinterRepository

```rust
trait PrinterRepository {
    fn save();
    fn find_all();
    fn find_by_name();
}
```

---

# 10. Application Layer

Application Layer chứa Use Case.

---

## CreatePrintJobUseCase

Flow

```text
Validate Request

Load Printer

Create Document

Create PrintJob

Persist

Publish Event
```

Output

```text
JobId
```

---

## RetryPrintJobUseCase

Flow

```text
Load Job

Validate Retry

Update Status

Push Queue
```

---

## CancelPrintJobUseCase

Flow

```text
Load Job

Validate Status

Cancel
```

---

## ListPrinterUseCase

Flow

```text
Load Printers

Return DTO
```

---

# 11. Application Events

Application Layer subscribe Domain Event.

Ví dụ:

```text
PrintJobCreated
```

Handler

```text
PushToQueueHandler
```

---

```text
PrintJobCompleted
```

Handler

```text
UpdateHistoryHandler
```

---

# 12. Interface Layer

## REST Controller

```text
POST /print-jobs

GET /print-jobs/{id}

POST /print-jobs/{id}/retry

POST /print-jobs/{id}/cancel

GET /printers
```

Controller chỉ:

```text
Validate DTO

Call UseCase

Return DTO
```

Không chứa Business Logic.

---

# 13. Queue Context

Queue không thuộc Domain.

Queue thuộc Infrastructure.

---

## QueueManager

```rust
struct QueueManager
```

Chức năng

```text
Push Job

Pop Job

Retry Job
```

---

## QueueWorker

```rust
struct QueueWorker
```

Flow

```text
Receive Job

Download

Render

Print

Update Status
```

---

# 14. Document Processing Context

## DocumentDownloader

```rust
trait DocumentDownloader
```

Implementation

```text
ReqwestDocumentDownloader
```

---

## DocumentRenderer

```rust
trait DocumentRenderer
```

Implementation

```text
PdfiumRenderer

ImageRenderer

ZplRenderer
```

---

# 15. Printer Context

## PrinterManager

```rust
trait PrinterManager
```

Chức năng

```text
List Printer

Get Default Printer

Check Printer Status
```

---

## PrinterEngine

```rust
trait PrinterEngine
```

Implementation

```text
WindowsPrinterEngine
```

---

# 16. Infrastructure Layer

## SQLite

Triển khai:

```text
PrintJobRepository

PrinterRepository
```

---

## PDFium

Triển khai:

```text
DocumentRenderer
```

---

## Windows API

Triển khai:

```text
PrinterEngine
```

---

## Reqwest

Triển khai:

```text
DocumentDownloader
```

---

# 17. Event Flow

## Create Job

```text
REST Request

↓

CreatePrintJobUseCase

↓

PrintJobCreated

↓

PushToQueueHandler

↓

Queue
```

---

## Print Success

```text
Worker

↓

PrintJobCompleted

↓

UpdateRepository

↓

Publish UI Event
```

---

# 18. Project Structure

```text
src-tauri/

├── interface
│   ├── rest
│   ├── websocket
│   └── tauri
│
├── application
│   ├── dto
│   ├── use_cases
│   ├── handlers
│   └── services
│
├── domain
│   ├── print_job
│   │   ├── aggregate.rs
│   │   ├── events.rs
│   │   ├── repository.rs
│   │   └── value_objects.rs
│   │
│   ├── printer
│   │   ├── aggregate.rs
│   │   ├── events.rs
│   │   └── repository.rs
│   │
│   └── document
│       ├── aggregate.rs
│       └── value_objects.rs
│
├── infrastructure
│   ├── database
│   ├── queue
│   ├── downloader
│   ├── renderer
│   ├── printer
│   └── eventbus
│
├── shared
│   ├── errors
│   ├── logger
│   └── config
│
└── main.rs
```

---

# 19. Cross Cutting Concerns

## Logging

```text
Tracing
```

---

## Metrics

```text
Job Count

Queue Size

Print Duration
```

---

## Error Handling

```text
ApplicationError

DomainError

InfrastructureError
```

---

## Configuration

```text
AppConfig

PrinterConfig

QueueConfig
```

---

# 20. Architectural Principles

1. Domain độc lập hoàn toàn.
2. Business Rule chỉ tồn tại trong Domain.
3. Use Case chỉ tồn tại trong Application.
4. Infrastructure chỉ triển khai Contract.
5. Không truy cập Database trực tiếp từ Controller.
6. Không in trực tiếp từ REST API.
7. Mọi Print Request đều tạo PrintJob.
8. Mọi thay đổi trạng thái đều phát Event.
9. Mọi Job đều đi qua Queue.
10. Printer là Aggregate riêng biệt.
11. Document là Aggregate riêng biệt.
12. Queue không thuộc Domain.
13. Infrastructure có thể thay thế mà không ảnh hưởng Domain.
14. Toàn bộ hệ thống tuân thủ Dependency Inversion Principle.
15. Toàn bộ luồng xử lý dựa trên Event-Driven Architecture.
