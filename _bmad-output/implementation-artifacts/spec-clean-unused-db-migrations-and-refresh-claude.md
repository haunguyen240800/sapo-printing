---
title: 'Clean unused database configuration schema and refresh project guidance'
type: 'refactor'
created: '2026-08-24'
status: 'done'
baseline_commit: '9dd15399104805241f6d1fc61316d69c6310e476'
context:
  - '{project-root}/CLAUDE.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** `migrations.rs` vẫn chứa persistence cấu hình máy in và nhiều seed `app_settings` không còn consumer, trong khi cấu hình in hiện đã được đọc/ghi bằng JSON. `CLAUDE.md` cũng đang mô tả kiến trúc dự kiến cũ, sai cấu trúc thư mục, API, renderer và cơ chế lưu cấu hình hiện tại.

**Approach:** Vì ứng dụng chưa phát hành và không cần tương thích database cũ, hợp nhất schema runtime cần dùng thành một baseline migration gọn, xóa toàn bộ persistence cấu hình không còn consumer, tinh gọn test theo schema cuối cùng, rồi viết lại `CLAUDE.md` dựa trên code hiện tại.

## Boundaries & Constraints

**Always:** Tạo đúng schema cho database mới; giữ các bảng/column/index đang có consumer (`print_jobs`, `events`, `api_tokens`, `app_settings.temp_file_retention_hours`); xác nhận cấu hình in dùng `~/.sapo-printer/print-config.json`; giữ nguyên các thay đổi version 1.0.9 đang có của người dùng.

**Ask First:** Bất kỳ thay đổi nào cần xóa dữ liệu job, event, token, thay đổi format/path JSON, hoặc sửa code ngoài `migrations.rs`, test liên quan và `CLAUDE.md`.

**Never:** Giữ schema/seed/test chỉ để tương thích database chưa phát hành; khôi phục SQLite printer repository; mô tả tính năng chỉ tồn tại trong planning artifact nhưng chưa có trong code; hoàn tác các thay đổi chưa commit không thuộc phạm vi.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|---------------|----------------------------|----------------|
| Cài mới | Database trống | Chạy baseline migration, chỉ tạo schema runtime cần dùng | Trả `MigrationFailed` nếu SQL lỗi |
| Chạy lại | Database đã chạy baseline migration | Không thay đổi schema hoặc dữ liệu hiện có | Migration idempotent qua version tracking |
| Cấu hình in | Save/load từ UI hoặc tạo print job | Đọc/ghi `print-config.json`, không truy cập `printer_configs` | Giữ hành vi lỗi hiện tại của JSON provider |

</frozen-after-approval>

## Code Map

- `src-tauri/src/infrastructure/configs/db/migrations.rs` — baseline schema SQLite tối giản và test schema cuối.
- `src-tauri/src/infrastructure/configs/app/app_print_config.rs` — nguồn sự thật về file `print-config.json`.
- `src-tauri/src/infrastructure/configs/app/json_file_config_provider.rs` — adapter cung cấp cấu hình JSON cho application use case.
- `src-tauri/src/infrastructure/temp_file.rs` — consumer duy nhất của `app_settings.temp_file_retention_hours`.
- `src-tauri/src/infrastructure/persistence/` — consumers của `print_jobs`, `events` và `api_tokens`.
- `src-tauri/src/bootstrap/` — composition root, database startup, worker và HTTP server hiện tại.
- `CLAUDE.md` — hướng dẫn repository cần đồng bộ với code thực tế.

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/src/infrastructure/configs/db/migrations.rs` — hợp nhất thành một baseline migration chỉ tạo schema/seed đang có consumer; xóa `printer_configs`, 6 `app_settings` không dùng và test lịch sử không còn ý nghĩa; giữ test schema, constraint và idempotency cần thiết.
- [x] `CLAUDE.md` — thay nội dung “expected” bằng kiến trúc, cấu trúc, command, API, persistence/config, nền tảng và giới hạn implementation hiện tại.

**Acceptance Criteria:**
- Given database trống, when `run_migrations` chạy một hoặc nhiều lần, then schema cuối ổn định và không có `printer_configs` hay app setting không dùng.
- Given source hiện tại, when tìm mọi SQL consumer, then mọi table/column/index còn lại trong migration đều có consumer hoặc ràng buộc vận hành rõ ràng.
- Given `CLAUDE.md` đã cập nhật, when đối chiếu với source/package manifests, then không còn tuyên bố PDFium/REST/WebSocket/cấu trúc thư mục dự kiến sai với implementation hiện tại.

## Spec Change Log

## Design Notes

Ứng dụng chưa phát hành nên migration history chưa phải compatibility contract. Một baseline duy nhất giúp schema khai báo phản ánh đúng runtime hiện tại và loại bỏ các bước `ALTER TABLE` chỉ phục vụ lịch sử phát triển; database local cũ của môi trường phát triển có thể cần được tạo lại thủ công.

## Verification

**Commands:**
- `cargo fmt --check --manifest-path src-tauri/Cargo.toml` — Rust formatting hợp lệ.
- `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::configs::db::migrations` — toàn bộ migration test pass.
- `cargo check --manifest-path src-tauri/Cargo.toml` — crate compile thành công.
- `rg -n "printer_configs|log_level|max_concurrent_downloads|max_concurrent_renders|default_batch_size|auto_update_enabled|last_update_check" src-tauri/src` — chỉ còn reference lịch sử/cleanup/test có chủ đích.

## Suggested Review Order

**Baseline database tối giản**

- Bắt đầu từ baseline duy nhất để thấy toàn bộ schema runtime còn lại.
  [`migrations.rs:6`](../../src-tauri/src/infrastructure/configs/db/migrations.rs#L6)

- App settings chỉ giữ key/value mà temp-file retention thực sự đọc.
  [`migrations.rs:7`](../../src-tauri/src/infrastructure/configs/db/migrations.rs#L7)

- Print jobs gom đủ column được repository và durable queue sử dụng.
  [`migrations.rs:14`](../../src-tauri/src/infrastructure/configs/db/migrations.rs#L14)

- Event audit và API token giữ schema/index phục vụ query hiện tại.
  [`migrations.rs:32`](../../src-tauri/src/infrastructure/configs/db/migrations.rs#L32)

**Hướng dẫn project hiện tại**

- Dependency rules phản ánh cả ngoại lệ Tauri và transition atomic của queue.
  [`CLAUDE.md:29`](../../CLAUDE.md#L29)

- Runtime flow ghi đúng JSON snapshot, queue worker và retry behavior.
  [`CLAUDE.md:81`](../../CLAUDE.md#L81)

- API ghi đúng fixed loopback port, Bearer auth và SSE endpoints.
  [`CLAUDE.md:97`](../../CLAUDE.md#L97)

- Persistence phân biệt SQLite job data với print-config JSON.
  [`CLAUDE.md:113`](../../CLAUDE.md#L113)

**Regression tests**

- Schema test khóa chính xác bốn application table và loại obsolete table.
  [`migrations.rs:75`](../../src-tauri/src/infrastructure/configs/db/migrations.rs#L75)

- Seed test khóa đúng retention key và giá trị mặc định 24 giờ.
  [`migrations.rs:98`](../../src-tauri/src/infrastructure/configs/db/migrations.rs#L98)
