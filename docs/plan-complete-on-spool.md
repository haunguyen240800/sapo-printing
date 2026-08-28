# Plan: Tính job in thành công ngay khi đẩy vào queue in của OS (bỏ chờ in xong vật lý)

> Trạng thái: **CHỜ REVIEW** — không implement cho tới khi được xác nhận.

## 1. Yêu cầu

Hiện tại `PrintJobCompleted` chỉ được sinh ra **sau khi máy in đã in xong vật lý** toàn bộ label. Yêu cầu: job **chỉ cần được đẩy (spool) vào queue in của OS thành công** thì đã tính là in thành công và phát `PrintJobCompleted` ngay.

**Quyết định phạm vi (đã chốt với người dùng):**

- **Giữ nguyên** luồng trạng thái `SubmittedToQueue → Printing → Completed` và state machine hiện tại.
- `PrintJobCompleted` phát **ngay sau khi spool xong** (không chờ in vật lý).
- **Dọn dead code**: xóa hẳn `wait_all_printed` ở trait + 3 platform impl và toàn bộ máy móc poll spooler chỉ phục vụ nó.

## 2. Hiện trạng (vì sao đang chờ in xong mới tính thành công)

`DefaultPrintService::print()` (`src-tauri/src/infrastructure/platform/printing.rs:31-54`) làm **2 việc**:

1. `render_strategy.render(...)` — spool từng trang. Trên Windows mỗi trang là 1 `StartDocW`/`EndDoc`, tức **commit tài liệu vào spooler OS = đẩy vào queue in**.
2. `backend.wait_all_printed()` (`printing.rs:50-52`) — **chặn (blocking)** cho tới khi spooler xác nhận mọi label đã **in xong vật lý** (hoặc lỗi/timeout).

Chính bước 2 là cái khiến "phải in xong mới tính thành công".

Chuỗi trạng thái trong `ProcessPrintJobUseCase::execute()` (`src-tauri/src/application/use_cases/process_print_job.rs:80-96`):

```
render_or_save()  // gọi print() = render (spool) + wait_all_printed (chờ in xong)
mark_submitted()  → SUBMITTED_TO_QUEUE
mark_printing()   → PRINTING
complete()        → COMPLETED  (+ event PrintJobCompleted)
```

Vì `wait_all_printed` nằm **bên trong** `print()`, nên tất cả các transition sau đó (submitted/printing/completed) chỉ chạy **sau khi** đã in xong vật lý.

## 3. Giải pháp

Bỏ `wait_all_printed()` khỏi `print()`. Khi đó `print()` trả về **ngay khi spool xong** = đẩy vào queue OS thành công. Chuỗi `mark_submitted → mark_printing → complete` chạy ngay sau đó, nên `PrintJobCompleted` phát ngay khi spool xong. **Không cần đổi** `process_print_job.rs`, `print_status.rs`, hay `aggregate.rs`.

Vì `wait_all_printed` không còn nơi gọi, xóa nó khỏi trait và mọi impl để không để lại dead code (theo quy tắc trong CLAUDE.md: không để hàm/field không dùng).

### Đánh đổi (cần người dùng ý thức khi review)

- **Mất khả năng phát hiện lỗi in vật lý sau spool**: hết giấy, offline, kẹt giấy, spooler báo lỗi sau khi đã nhận job → job vẫn được tính `COMPLETED`. Đây đúng là hành vi mong muốn ("đẩy vào queue OS = xong"), nhưng cần xác nhận.
- Lỗi **spool** (không commit được vào queue, ví dụ `EndDoc` lỗi, máy in không mở được DC) **vẫn** làm job `FAILED` vì `render()` trả `Err`. Chỉ mất phần chờ **sau** spool.
- `SPOOL_STALL_TIMEOUT` 30s (chờ in) không còn → worker nhả job nhanh hơn, không bị treo chờ máy in.

## 4. Chi tiết triển khai

### Bước 1 — `printing.rs`: bỏ chờ in vật lý

File: `src-tauri/src/infrastructure/platform/printing.rs`

Trong `PrintPort::print`, bỏ khối `wait_all_printed()` (dòng ~45-52), chỉ giữ `render()`:

```rust
fn print(
    &self,
    pdf_path: &str,
    printer_name: &str,
    settings: &PrintJobSettings,
) -> Result<(), Error> {
    let mut backend = GraphicsBackendFactory::create();
    // Document lifecycle (begin_document / end_document) is managed inside
    // render() on a per-page basis so each label is a separate spooler job.
    // A job is considered done once every page is committed to the OS spooler
    // (queue); we intentionally do NOT wait for physical printing to finish.
    self.render_strategy
        .render(pdf_path, printer_name, settings, &mut *backend)?;
    Ok(())
}
```

Lưu ý: sau khi bỏ, kiểm tra `InfrastructureError` còn được dùng trong file không — **còn** (dùng ở `save_to_path`), nên giữ import.

### Bước 2 — `backend.rs`: bỏ method khỏi trait

File: `src-tauri/src/infrastructure/platform/printer_api/backend.rs`

Xóa dòng khỏi trait `GraphicsBackend`:

```rust
fn wait_all_printed(&mut self) -> Result<(), String>;
```

### Bước 3 — `windows.rs`: xóa impl + toàn bộ máy móc poll spooler

File: `src-tauri/src/infrastructure/platform/printer_api/windows.rs`

Xóa các phần **chỉ** phục vụ `wait_all_printed`:

- `impl` method `wait_all_printed` (dòng ~528-534).
- Helper `wait_for_all` (dòng ~63-146) và `query_job_status` (dòng ~148-168).
- `enum JobQuery` (dòng ~35-41).
- Các hằng `JS_ERROR/JS_OFFLINE/JS_PAPEROUT/JS_PRINTED/JS_DELETED/JS_BLOCKED_DEVQ/JS_USER_INTERVENTION` (dòng ~19-25).
- Hằng `SPOOL_STALL_TIMEOUT`, `SPOOL_POLL_INTERVAL` (dòng ~31-33).
- Field struct `spooled_job_ids`, `printer_name`, và `current_job_id` (dòng ~46-50) + khởi tạo trong `new()`.
  - `begin_document`: bỏ gán `self.current_job_id = ...` và `self.printer_name = ...` (dòng ~349-350) cùng comment liên quan (~346-348). Lưu ý `result` của `StartDocW` vẫn cần cho check lỗi `result <= 0`.
  - `end_document`: bỏ `let job_id = self.current_job_id.take();` và khối `if let Some(id) = job_id { self.spooled_job_ids.push(id); }` (dòng ~500, ~510-512).
  - `abort_document`: bỏ dòng `self.current_job_id = None;` (dòng ~525).
- Dọn import không còn dùng: `GetJobW`, `JOB_INFO_2W` (dòng ~9-11). **Giữ** `OpenPrinterW`, `ClosePrinter`, `DocumentPropertiesW`, `HANDLE` (còn dùng trong `build_devmode`).

### Bước 4 — `macos.rs` và `linux.rs`: xóa impl no-op

Files:
- `src-tauri/src/infrastructure/platform/printer_api/macos.rs` (dòng ~175-179)
- `src-tauri/src/infrastructure/platform/printer_api/linux.rs` (dòng ~176-180)

Cả hai chỉ là no-op `Ok(())`, xóa hẳn method. Không có field/helper phụ thuộc.

### Bước 5 — Kiểm tra

- `cargo check --manifest-path src-tauri/Cargo.toml` (build target Windows).
- `cargo test --manifest-path src-tauri/Cargo.toml` — không có test nào gọi trực tiếp `wait_all_printed` (đã grep). Nếu có test tự viết mock `GraphicsBackend`, phải bỏ method khỏi mock.
- `cargo fmt --check`.

## 5. Không thay đổi (khẳng định)

- `process_print_job.rs`: **giữ nguyên** — luồng submit → printing → complete không đổi.
- `print_status.rs` (state machine, enum `PrintStatus`, transitions): **giữ nguyên** — vẫn có `SubmittedToQueue`, `Printing`, và các transition.
- `aggregate.rs` (`mark_submitted`, `mark_printing`, `complete`): **giữ nguyên**.
- Event `PrintJobPrinting`, mapping progress ở `print_job_response.rs` / event emitter: **giữ nguyên**.

## 6. File đụng tới (tổng kết)

| File | Thay đổi |
|---|---|
| `infrastructure/platform/printing.rs` | Bỏ gọi `wait_all_printed` |
| `infrastructure/platform/printer_api/backend.rs` | Bỏ method khỏi trait |
| `infrastructure/platform/printer_api/windows.rs` | Xóa impl + `wait_for_all` + `query_job_status` + `JobQuery` + hằng JS_*/SPOOL_* + field `spooled_job_ids`/`printer_name`/`current_job_id` + import thừa |
| `infrastructure/platform/printer_api/macos.rs` | Xóa impl no-op |
| `infrastructure/platform/printer_api/linux.rs` | Xóa impl no-op |
