# CLAUDE.md

Tài liệu này mô tả trạng thái **đang được triển khai trong source code** của repository. Các tài liệu BMad trong `_bmad-output/` là lịch sử lập kế hoạch và có thể không còn phản ánh runtime hiện tại; khi có khác biệt, ưu tiên code và manifest.

## Tổng quan

**Sapo Printer Pro Max** là ứng dụng desktop Tauri 2 nhận yêu cầu in từ webapp SAPO, tải PDF, lưu print job bền vững trong SQLite, render bằng PDFium và gửi sang hệ thống in của OS. Frontend sử dụng React 18, TypeScript, Vite và `@sapo/ui-components`; backend sử dụng Rust edition 2024.

Ứng dụng hiện chưa phát hành. Database development cũ không cần được hỗ trợ khi baseline migration thay đổi và có thể được tạo lại.

## Lệnh thường dùng

```bash
pnpm install
pnpm dev                 # Tauri app + Vite frontend
pnpm run dev:web         # chỉ frontend
pnpm run build:web       # TypeScript check + Vite build
pnpm lint
pnpm format:check

cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
pnpm build               # production Tauri build
```

Toolchain được pin trong `package.json`: Node 26.5.0 và pnpm 11.17.0. Bundle hiện tại là Windows NSIS cài theo user và đóng gói `src-tauri/bin/pdfium.dll`.

## Kiến trúc hiện tại

Backend tuân theo Clean Architecture với dependency direction:

```text
Interface (Axum HTTP/SSE, Tauri commands)
        -> Application (models, use cases, handlers, ports)
        -> Domain (PrintJob aggregate, value objects, events, repository contract)
Infrastructure implements Application ports và Domain repository contract
Bootstrap tạo dependency graph và khởi động runtime
```

Quy tắc bắt buộc:

1. `domain` không phụ thuộc Application, Infrastructure, Interface hoặc Tauri.
2. Use case và orchestration nằm trong `application`; business state transition nằm trong `domain`.
3. HTTP print-job handlers chỉ chuyển đổi input-output và gọi use case. Một số Tauri command hiện vẫn tự validate và gọi adapter/plugin trực tiếp; không mở rộng pattern này sang print-job flow.
4. Mọi print request phải tạo `PrintJob`, persist job/event rồi đi qua `QueuePort` và `QueueWorker`.
5. Adapter SQLite, HTTP download, PDFium và printer API nằm trong `infrastructure`.
6. Các transition do use case điều khiển sinh domain event và persist event trước khi publish in-memory. Durable queue hiện claim `PENDING -> QUEUED -> PROCESSING` trực tiếp trong SQLite để bảo đảm atomicity và chưa phát event riêng cho các bước này.
7. Không thêm lại `PrinterRepository` hoặc bảng `printer_configs`: cấu hình in được lưu bằng JSON.

## Cấu trúc source

```text
src/                                      # React frontend
  components/ pages/ services/ types/

src-tauri/src/
  domain/print_job/                       # aggregate, status, settings, events, repository trait
  application/
    models/                               # DTO cho HTTP/Tauri
    ports/                                # Config, printer, print, queue, event, token, temp-file ports
    use_cases/                            # create/process/status/metrics/audit/list printers
    handlers/                             # handlers cho job created/failed
  infrastructure/
    configs/app/                          # print-config.json + ConfigPort adapter
    configs/db/                           # SQLite pool và baseline migration
    persistence/                          # print jobs, queue, events, API tokens
    integrations/network/                 # Reqwest downloader + circuit breaker
    integrations/pdf_engine/              # PDFium bitmap renderer
    platform/                             # printer APIs, spooler, updater, port binder
    telemetry/                            # structured logging và metrics
    worker/                               # QueueWorker polling SQLite queue
  interface/
    http_server/                          # Axum REST API, auth middleware, CORS, SSE
    tauri/                                # desktop commands và event emitter
  bootstrap/                              # dirs, DB, DI, HTTP server, tray, updater
  lib.rs                                  # Tauri builder và command registration
  main.rs                                 # binary entry point
```

## Print job runtime

`CreatePrintJobUseCase` thực hiện:

1. Validate `document_url`.
2. Đọc snapshot cấu hình từ JSON qua `ConfigPort`.
3. Kiểm tra máy in đang online qua `PrinterPort`.
4. Tạo và persist `PrintJob` cùng `PrintJobCreated` event.
5. Publish event; `PrintJobCreatedHandler` đẩy job vào durable SQLite queue.

`QueueWorker` poll mỗi 500 ms, claim job và gọi `ProcessPrintJobUseCase`: download PDF -> quản lý temp file -> render/in -> persist từng state/event -> complete. Cấu hình giấy/màu đã được snapshot vào `print_jobs.settings_json`, vì vậy worker không đọc lại config mới giữa chừng.

Các trạng thái canonical trong SQLite là `PENDING`, `QUEUED`, `PROCESSING`, `DOWNLOADED`, `SUBMITTED_TO_QUEUE`, `PRINTING`, `COMPLETED`, `FAILED`, `CANCELLED`.

Pipeline hiện **không tự retry**: `PrintJobFailedHandler` đánh dấu lỗi vĩnh viễn và lưu/publish event. Domain vẫn có `retry()` với giới hạn 3 lần nhưng chưa có retry use case/API được wire vào runtime.

## HTTP loopback API

Server Axum dùng plain HTTP cố định tại `127.0.0.1:18901`. Không fallback sang port khác. Metadata runtime được ghi vào `agent.json`.

```text
GET  /api/v1/ping          public
POST /api/v1/pair          public, yêu cầu Origin hợp lệ và xác nhận trên UI
GET  /api/v1/events        SSE, token qua query parameter
POST /api/v1/jobs          Bearer token, tạo print job
GET  /api/v1/jobs/:id      Bearer token, lấy trạng thái job
```

Real-time update dùng **Server-Sent Events**, không phải WebSocket. Hiện SSE broadcast `PrintJobCompleted` và `PrintJobFailed`.

Tauri commands phục vụ UI desktop gồm printer discovery/config/status/category, metrics, pairing approval, auto-start và update lifecycle. Danh sách authoritative nằm trong `src-tauri/src/lib.rs`.

## Persistence và file cấu hình

- Data root là **thư mục cài đặt (cạnh file exe)** để mọi Windows user dùng chung DB/config/logs. `bootstrap/dirs.rs` resolve root bằng `std::env::current_exe()` trong release; debug build fallback về OS data dir (`<data_dir>/sapo-printer-pro-max`) để không làm bẩn `target/`. SQLite nằm tại `<install_dir>/config.db`.
- SQLite chỉ giữ `print_jobs`, `events`, `api_tokens` và `app_settings`.
- `app_settings` hiện chỉ có `temp_file_retention_hours`; không thêm setting không có consumer.
- SQLite, logs, temporary downloads, `agent.json` và cấu hình in (`<install_dir>/print-config.json`) đều nằm dưới data root; mọi path runtime được tạo trong `bootstrap/dirs.rs` rồi inject vào adapter, không tự dựng từ `HOME`/`USERPROFILE`.
- Print-job settings được serialize riêng vào `print_jobs.settings_json` để giữ snapshot tại thời điểm tạo job.
- Không có secret store: app không dùng OS keychain và không còn HMAC signing key. Vì DB dùng chung cho mọi user, key nằm cạnh dữ liệu sẽ vô nghĩa cho việc chống giả mạo; audit trail chỉ lưu lịch sử event (bảng `events`), không ký HMAC.
- Logs và temp files nằm dưới data root; temp retention mặc định là 24 giờ.

Vì cài `perMachine` và data nằm cạnh exe: khuyến nghị cài ổ D/E (root ổ non-system cho Users quyền ghi mặc định). Cài ổ C (`Program Files` hoặc root C) thì user thường không ghi được DB → phải chạy admin. Không có NSIS hook cấp quyền ghi (giống mô hình BigSeller).

Các chuỗi tên slug được giữ có chủ đích: binary là `sapo-printer`, crate/tracing namespace là `sapo_printer`, bundle identity là `com.sapo.printer`, còn frontend localStorage dùng `sapo-printer.pending-update-version`. Đây là technical identity/compatibility key, không được đổi chỉ để đồng nhất cách viết tên sản phẩm.

Khi chỉnh `migrations.rs`, phải đối chiếu mọi table/column/index với SQL consumer trong `infrastructure/persistence`, `infrastructure/telemetry` và `temp_file.rs`. Vì app chưa phát hành, baseline có thể được gộp lại; sau khi đổi baseline cần tạo lại database development đã migrate bằng schema cũ.

## Printing và platform

- PDF được tải bằng blocking Reqwest adapter có circuit breaker.
- Production dùng `BitmapRenderStrategy` dựa trên `pdfium-render`.
- Feature Cargo `native_pdf_render` chưa hoàn thiện và mặc định tắt; không bật trong production.
- `DefaultPrintService` tạo graphics backend theo OS, spool từng trang và chờ spooler xác nhận toàn bộ label.
- `ProcessPrintJobUseCase` có nhánh bypass spooler bằng cách copy PDF tới `output_path`, nhưng entry point tạo job hiện luôn truyền `None`; nhánh virtual-PDF này chưa được wire vào request runtime.
- Printer discovery/backend có implementation Windows, macOS và Linux, nhưng bundle/release configuration hiện tập trung vào Windows.

## Testing và chất lượng

- Rust unit/integration tests nằm cạnh module và dùng SQLite in-memory hoặc temp file khi cần WAL.
- Migration tests phải xác nhận baseline idempotent, chỉ tạo schema runtime cần dùng và giữ các constraint/index mà repository dựa vào.
- Frontend bắt buộc qua TypeScript build, ESLint và Prettier check.
- Dùng `tracing` cho structured logs; không thêm `println!`, `dbg!`, `todo!`, `unimplemented!` hoặc `unwrap()` vào non-test code.
- Không xóa/ghi đè thay đổi chưa commit ngoài phạm vi task.

## BMad

- Planning artifacts: `_bmad-output/planning-artifacts/`
- Implementation artifacts: `_bmad-output/implementation-artifacts/`
- Test artifacts: `_bmad-output/test-artifacts/`
- Cấu hình BMM: `_bmad/bmm/config.yaml`

Ngôn ngữ giao tiếp và tài liệu dự án là tiếng Việt. Story/spec cũ mô tả ý định tại thời điểm lập kế hoạch, không phải contract chính xác hơn source hiện tại.
