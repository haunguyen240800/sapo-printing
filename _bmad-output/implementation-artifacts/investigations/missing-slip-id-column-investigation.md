# Investigation: SQLite thiếu cột slip_id

## Hand-off Brief

1. **What happened.** Confirmed: QueueWorker dùng SQL mới có `slip_id`, nhưng database debug đã migrate baseline version 1 cũ nên thiếu cột.
2. **Where the case stands.** Concluded với confidence High; baseline source hiện đã đúng và không cần migration nâng cấp riêng vì app chưa public.
3. **What's needed next.** Dừng app, xóa trọn bộ ba file SQLite development rồi khởi động lại để baseline tạo schema mới.

## Case Info

| Field | Value |
| --- | --- |
| Ticket | N/A |
| Date opened | 2026-08-28 |
| Status | Concluded |
| System | Windows development, Sapo Printer Pro Max |
| Evidence sources | Runtime log, source code, Git diff và SQLite runtime read-only |

## Problem Statement

Từ `2026-08-28T08:02:39` đến ít nhất `08:02:44`, QueueWorker không thể pop job vì query mới chọn `slip_id` nhưng SQLite báo database đang mở không có cột này.

## Evidence Inventory

| Source | Status | Notes |
| --- | --- | --- |
| Runtime log 2026-08-28 08:02:39–08:02:44 | Available | Lỗi tái diễn mỗi khoảng 500 ms tại QueueWorker |
| Baseline migration source | Available | `slip_id` và index nằm trong `MIGRATION_1` |
| Runtime database file/schema | Available | Debug DB tại AppData Roaming, `user_version = 1`, thiếu `slip_id` |
| Git diff/history | Available | Working diff thêm cột/index trực tiếp vào baseline |

## Investigation Backlog

| # | Path to Explore | Priority | Status | Notes |
| - | --- | --- | --- | --- |
| 1 | SQL query pop queue và baseline migration | High | Done | Consumer và baseline mới đều yêu cầu `slip_id` |
| 2 | Migration runner và version tracking | High | Done | Chỉ có `MIGRATION_1`; runtime DB đã ở user_version 1 |
| 3 | Runtime data-root/config.db | High | Done | Đã xác định AppData Roaming và ba file SQLite WAL |
| 4 | Git diff thêm `slip_id` | Medium | Done | Cột/index được thêm trực tiếp vào baseline theo quyết định pre-release |

## Timeline of Events

| Time | Event | Source | Confidence |
| --- | --- | --- | --- |
| Trước 2026-08-28 | Database development chạy baseline version 1 chưa có `slip_id` | Runtime SQLite PRAGMA | Confirmed |
| 2026-08-28T08:02:39.822296Z | QueueWorker thất bại vì query yêu cầu `slip_id` | Runtime log | Confirmed |
| 2026-08-28T08:02:40–08:02:44Z | Lỗi lặp lại theo chu kỳ poll khoảng 500 ms | Runtime log | Confirmed |
| 2026-08-28 | User xác nhận app chưa public và chọn reset development data | User decision | Confirmed |

## Confirmed Findings

### Finding 1: SQL consumer yêu cầu cột không tồn tại trong database runtime

**Evidence:** Runtime log `2026-08-28T08:02:39.822296Z`; `src-tauri/src/infrastructure/persistence/job_queue_broker.rs:101`.

**Detail:** Query `SELECT id, slip_id, ... FROM print_jobs` thất bại trước khi QueueWorker claim được job.

### Finding 2: Baseline source hiện đã chứa cột và index

**Evidence:** `src-tauri/src/infrastructure/configs/db/migrations.rs:16`, `src-tauri/src/infrastructure/configs/db/migrations.rs:32`, `src-tauri/src/infrastructure/configs/db/migrations.rs:59`.

**Detail:** `MIGRATION_1` tạo `slip_id TEXT NOT NULL DEFAULT ''`, tạo `idx_print_jobs_slip_id`, và là migration duy nhất được đăng ký.

### Finding 3: Database debug cũ đã đánh dấu baseline version 1 nhưng thiếu cột

**Evidence:** Read-only SQLite diagnostic ngày 2026-08-28 trên `C:\Users\HauNV-PC\AppData\Roaming\sapo-printer-pro-max\config.db`.

**Detail:** `PRAGMA user_version = 1`; `PRAGMA table_info(print_jobs)` không có `slip_id`. Cạnh DB tồn tại cả `config.db-wal` và `config.db-shm`.

## Deduced Conclusions

### Deduction 1: Migration runner không chạy lại baseline đã applied

**Based on:** Finding 2 và Finding 3.

**Reasoning:** Source và runtime DB cùng mang migration version 1; việc sửa nội dung `MIGRATION_1` không làm `to_latest()` thực thi lại version đã applied.

**Conclusion:** DB cũ giữ schema cũ cho tới khi được reset hoặc có migration version mới.

### Deduction 2: Print queue bị ngừng xử lý hoàn toàn

**Based on:** Finding 1 và lỗi lặp lại theo poll interval.

**Reasoning:** `pop()` không trả được job vì câu SELECT không compile; worker chỉ log lỗi rồi poll lại.

**Conclusion:** Không job nào tiến vào pipeline cho tới khi DB runtime được tạo lại với baseline mới.

## Hypothesized Paths

### Hypothesis 1: Database development đã chạy baseline cũ

**Status:** Confirmed

**Theory:** Cột `slip_id` được thêm bằng cách sửa migration baseline version 1; database hiện tại đã đánh dấu version đó nên migration runner không chạy lại DDL.

**Supporting indicators:** SQL consumer mới chạy, nhưng schema runtime thiếu đúng cột mới.

**Would confirm:** Migration source chỉ có baseline đã chỉnh sửa và DB runtime đã ghi baseline version 1.

**Would refute:** Có migration version mới đã chạy trên đúng DB runtime.

**Resolution:** Findings 2–3 xác nhận đầy đủ giả thuyết.

## Missing Evidence

| Gap | Impact | How to Obtain |
| --- | --- | --- |
| Không còn gap chặn kết luận | Không áp dụng | Không áp dụng |

## Source Code Trace

| Element | Detail |
| --- | --- |
| Error origin | `src-tauri/src/infrastructure/persistence/job_queue_broker.rs:101` |
| Trigger | QueueWorker poll và gọi queue pop |
| Condition | Bảng `print_jobs` thiếu cột `slip_id` |
| Migration | `src-tauri/src/infrastructure/configs/db/migrations.rs:59` chỉ đăng ký `MIGRATION_1` |
| Runtime path | `src-tauri/src/bootstrap/dirs.rs:35` tạo `<data_root>/config.db` |
| Migration startup | `src-tauri/src/bootstrap/database.rs:22` gọi `run_migrations` |

## Conclusion

**Confidence:** High

Root cause confirmed: database debug đã applied baseline version 1 cũ; source sửa chính baseline version 1 để thêm `slip_id`, nên migration runner coi DB đã latest và không chạy lại `CREATE TABLE`. Quyết định giữ thay đổi trong baseline phù hợp với trạng thái pre-release; cần reset database development.

## Recommended Next Steps

### Fix direction

Dừng app hoàn toàn, xóa `config.db`, `config.db-wal`, `config.db-shm` dưới `C:\Users\HauNV-PC\AppData\Roaming\sapo-printer-pro-max`, rồi khởi động lại. Không thêm migration `ALTER TABLE` riêng.

### Diagnostic

Sau restart, xác nhận log không còn `no such column`; migration test hiện đã xác nhận cột và index trên DB sạch.

## Reproduction Plan

1. Dừng app và xóa ba file SQLite development.
2. Khởi động app để `MIGRATION_1` tạo DB mới.
3. Tạo print job có `slip_id`; QueueWorker phải pop được job mà không log lỗi schema.

## Side Findings

- SQLite runtime đang dùng WAL mode, nên chỉ xóa `config.db` trong khi app còn chạy là không đủ và có nguy cơ dữ liệu được phục hồi từ WAL.

## Follow-up: 2026-08-28

### New Evidence

- User xác nhận app chưa public và chọn giữ `slip_id` trong baseline, tự reset data development.
- Baseline source và runtime DB được đối chiếu; runtime DB thiếu cột dù `user_version = 1`.

### Additional Findings

- Debug data-root resolve tới OS data dir + slug; trên máy hiện tại là AppData Roaming.
- SQLite đang ở WAL mode nên reset phải gồm file chính, WAL và SHM sau khi dừng app.

### Updated Hypotheses

- Hypothesis 1 chuyển từ Open sang Confirmed.

### Backlog Changes

- Tất cả investigation backlog đã Done.

### Updated Conclusion

Case concluded, confidence High. Không cần code change bổ sung; thao tác còn lại là reset database development.
