---
title: 'Đổi tên CancelSlipJobsUseCase thành CancelPrintJobUseCase'
type: 'refactor'
created: '2026-08-28'
status: 'done'
baseline_commit: '38a8d12ba1679af29d43a9eb21af1f85758f6a37'
context:
  - '{project-root}/CLAUDE.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Use case hủy print job đang mang tên `CancelSlipJobsUseCase`, khiến tên type và các định danh wiring không thống nhất với thuật ngữ `PrintJob` của application/domain.

**Approach:** Đổi tên type thành `CancelPrintJobUseCase` và đổi đồng bộ các định danh kỹ thuật trực tiếp đại diện cho use case, nhưng không thay đổi hành vi hủy theo `slip_id`, HTTP contract hay logic persistence/event hiện có.

## Boundaries & Constraints

**Always:** Giữ nguyên nội dung working tree chưa commit; đổi đồng bộ tên file/module, type, re-export, dependency wiring, field/local variable, tracing target/message và comment tham chiếu trực tiếp tới use case; bảo đảm không còn định danh cũ trong source runtime.

**Ask First:** Mọi thay đổi làm đổi request/response HTTP, route, tham số `slip_id`, thuật toán hủy nhiều job hoặc thứ tự persist/publish event.

**Never:** Không đổi business behavior, schema/database, domain transition, public HTTP endpoint `/api/v1/jobs/cancel`, DTO `CancelSlipRequest`/`CancelSlipResponse` hay handler `cancel_slip`; không sửa các thay đổi đang có ngoài phạm vi rename.

</frozen-after-approval>

## Code Map

- `src-tauri/src/application/use_cases/cancel_slip_jobs.rs` — định nghĩa use case, tracing target/message; đổi thành module/file `cancel_print_job.rs`.
- `src-tauri/src/application/use_cases/mod.rs` — khai báo module và re-export type.
- `src-tauri/src/application/use_cases/process_print_job.rs` — comment tham chiếu type cũ.
- `src-tauri/src/bootstrap/app_state.rs` — khởi tạo dependency trong application context.
- `src-tauri/src/bootstrap/http_server.rs` — chuyển dependency từ context sang HTTP bootstrap.
- `src-tauri/src/interface/http_server/bootstrap.rs` — tham số bootstrap và construction của HTTP state.
- `src-tauri/src/interface/http_server/state.rs` — type/field dependency trong `AppState`.
- `src-tauri/src/interface/http_server/handlers.rs` — lấy use case từ state để thực thi request hủy.
- `src-tauri/src/lib.rs` — re-export/import và field trong context dùng chung.

## Tasks & Acceptance

**Execution:**

- [x] Đổi `cancel_slip_jobs.rs` thành `cancel_print_job.rs`, đổi module/type/impl/tracing/comment sang `CancelPrintJobUseCase` và `cancel_print_job`.
- [x] Đổi các field/local variable wiring trực tiếp từ `cancel_slip_jobs_uc`/`cancel_slip_uc` thành `cancel_print_job_uc` trên application context, bootstrap và HTTP state/handler.
- [x] Rà toàn bộ source để loại bỏ `CancelSlipJobsUseCase`, `cancel_slip_jobs` và các tên biến wiring cũ, không đụng đến contract HTTP vẫn mô tả thao tác hủy theo slip.

**Acceptance Criteria:**

- Given source đã được đổi tên, when tìm kiếm các định danh cũ `CancelSlipJobsUseCase`, `cancel_slip_jobs`, `cancel_slip_jobs_uc` và `cancel_slip_uc`, then không còn kết quả trong source runtime.
- Given request `POST /api/v1/jobs/cancel` với `slip_id` hợp lệ, when handler được biên dịch sau refactor, then request vẫn gọi cùng logic hủy và trả cùng cấu trúc response.
- Given working tree có thay đổi chưa commit trước task, when hoàn tất refactor, then các thay đổi đó vẫn được giữ nguyên ngoài các dòng cần thiết cho rename.

## Spec Change Log

## Verification

**Commands:**

- `rg -n "CancelSlipJobsUseCase|cancel_slip_jobs|cancel_slip_jobs_uc|cancel_slip_uc" src-tauri/src` — expected: không có kết quả.
- `cargo fmt --check --manifest-path src-tauri/Cargo.toml` — expected: formatting hợp lệ.
- `cargo check --manifest-path src-tauri/Cargo.toml` — expected: Rust backend biên dịch thành công.
- `cargo test --manifest-path src-tauri/Cargo.toml` — expected: toàn bộ Rust tests vượt qua.

## Suggested Review Order

**Định danh use case**

- Bắt đầu tại type mới và xác nhận logic execute chỉ được đổi tên.
  [`cancel_print_job.rs:15`](../../src-tauri/src/application/use_cases/cancel_print_job.rs#L15)

- Module và re-export công khai dùng cùng định danh mới.
  [`mod.rs:1`](../../src-tauri/src/application/use_cases/mod.rs#L1)

**Dependency wiring**

- Application context sở hữu dependency dưới field mới.
  [`lib.rs:16`](../../src-tauri/src/lib.rs#L16)

- Bootstrap khởi tạo đúng type mới với các dependency cũ.
  [`app_state.rs:145`](../../src-tauri/src/bootstrap/app_state.rs#L145)

- HTTP state truyền use case qua tên wiring thống nhất.
  [`state.rs:18`](../../src-tauri/src/interface/http_server/state.rs#L18)

- Handler dùng field mới nhưng giữ nguyên contract hủy slip.
  [`handlers.rs:212`](../../src-tauri/src/interface/http_server/handlers.rs#L212)

**Tham chiếu hỗ trợ**

- Cancel-gate chỉ cập nhật comment sang tên type mới.
  [`process_print_job.rs:63`](../../src-tauri/src/application/use_cases/process_print_job.rs#L63)
