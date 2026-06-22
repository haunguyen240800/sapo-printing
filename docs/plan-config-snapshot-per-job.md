# Plan: Snapshot cấu hình vào từng PrintJob (job giữ đúng cấu hình lúc tạo)

> Trạng thái: **CHỜ REVIEW** — không implement cho tới khi được xác nhận.

## 1. Yêu cầu

Các job được tạo **trước** thời điểm người dùng đổi cấu hình máy in phải in theo **cấu hình cũ** (cả máy in đích lẫn thông số in: khổ giấy, hướng, lề, màu, `print_as_image`, dpi, copies, rotate). Job tạo **sau** khi đổi thì theo cấu hình mới.

## 2. Hiện trạng (vì sao chưa đạt)

Một "cấu hình" gồm 2 phần:

| Thành phần | Snapshot theo lúc tạo job? | Ghi chú |
|---|---|---|
| Máy in đích | ✅ Có | `job.printer_id` đóng băng ở `create_print_job.rs:41`, lúc in dùng `job.printer_id()` (`process_print_job.rs:118`) |
| Thông số in | ❌ Không | Bị mất khi lưu DB, và bị ghi đè bằng cấu hình **hiện tại** lúc in |

Nguyên nhân chi tiết với **thông số in**:

1. **Không lưu** — Bảng `print_jobs` không có cột nào chứa settings (`migrations.rs`: MIGRATION_3 + 5/6/7 chỉ có `printer_name`, `status`, `retry_count`, timestamps, `scheduled_at`, `error_message`, `output_path`). Khi `PrintJobRepository::save()` chạy, `job.settings` bị bỏ qua.
2. **Không khôi phục** — Khi dựng lại job:
   - `PrintJobRepository::row_to_print_job()` (`print_job_repository.rs:374`) dùng `PrintJobSettings::default()`.
   - `JobQueueBroker::pop()` (`job_queue_broker.rs:138`) cũng dùng `PrintJobSettings::default()`.
3. **In theo cấu hình hiện tại** — `ProcessPrintJobUseCase::render_or_save()` (`process_print_job.rs:110-115`) gọi lại `config_provider.load_print_config()` để lấy settings tại thời điểm in.

→ Hệ quả: job tạo trước khi đổi cấu hình khi in ra sẽ dùng **máy in cũ nhưng khổ giấy/hướng/màu của cấu hình mới**.

Lưu ý: phía tạo job đã snapshot đúng rồi (`create_print_job.rs:69` `let settings: PrintJobSettings = (&config).into();` rồi truyền vào `PrintJob::new_with_output_path`). Vấn đề chỉ ở **persistence + khôi phục + dùng lúc in**.

## 3. Giải pháp tổng quát

Lưu snapshot `PrintJobSettings` (đã là `Serialize`/`Deserialize`, xem `value_objects/settings.rs`) xuống DB dưới dạng JSON, khôi phục đúng khi đọc/pop, và lúc in dùng `job.settings` thay vì nạp lại cấu hình hiện tại.

Gồm 4 thay đổi bắt buộc (Bước 1–4) + 1 dọn dẹp tùy chọn (Bước 5) + 1 bản vá chống-treo khuyến nghị đi kèm (Bước 6).

---

## 4. Chi tiết triển khai

### Bước 1 — Migration thêm cột `settings_json`

File: `src-tauri/src/infrastructure/configs/db/migrations.rs`

Thêm hằng mới sau `MIGRATION_8`:

```rust
const MIGRATION_9: &str = "
ALTER TABLE print_jobs ADD COLUMN settings_json TEXT;
";
```

Thêm vào danh sách migrations trong `run_migrations()`:

```rust
    let migrations = Migrations::new(vec![
        M::up(MIGRATION_1),
        M::up(MIGRATION_2),
        M::up(MIGRATION_3),
        M::up(MIGRATION_4),
        M::up(MIGRATION_5),
        M::up(MIGRATION_6),
        M::up(MIGRATION_7),
        M::up(MIGRATION_8),
        M::up(MIGRATION_9), // NEW
    ]);
```

Ghi chú: cột nullable → các row cũ (tạo trước migration) có `settings_json = NULL`, sẽ fallback về `PrintJobSettings::default()` khi đọc (an toàn, không phá dữ liệu cũ).

### Bước 2 — Ghi snapshot khi `save()`

File: `src-tauri/src/infrastructure/persistence/print_job_repository.rs`, hàm `save()`.

Trước khi `conn.execute(...)`, serialize settings:

```rust
let settings_json = serde_json::to_string(&job.settings).map_err(|e| {
    PrintJobError::RepositoryError {
        reason: format!("Failed to serialize job settings: {}", e),
    }
})?;
```

Sửa câu INSERT để thêm cột + tham số `?11`:

```rust
let rows = conn
    .execute(
        "INSERT INTO print_jobs (id, printer_name, document_url, status, retry_count, created_at, updated_at, completed_at, error_message, output_path, settings_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            job.id().to_string(),
            job.printer_id().as_str(),
            job.pdf_url(),
            job.status().to_db_string(),
            job.retry_count() as i64,
            now,
            now,
            completed_at,
            job.error_message(),
            job.output_path(),
            settings_json, // NEW
        ],
    )
    .map_err(|e| { /* giữ nguyên khối xử lý lỗi hiện tại */ })?;
```

`update()` **không cần đổi** — settings là bất biến trong vòng đời job, chỉ ghi 1 lần lúc tạo.

### Bước 3 — Khôi phục snapshot khi đọc từ DB

File: `src-tauri/src/infrastructure/persistence/print_job_repository.rs`.

**3a.** Thêm `settings_json` vào 3 câu SELECT (`find_by_id`, `find_by_status`, `find_all`) — thêm vào **cuối** danh sách cột để giữ index các cột cũ:

```sql
SELECT id, printer_name, document_url, status, retry_count, created_at, completed_at, error_message, output_path, settings_json
FROM print_jobs ...
```

**3b.** Sửa `row_to_print_job()` (hiện đọc cột 0..8, dùng default ở dòng 374):

```rust
fn row_to_print_job(row: &rusqlite::Row<'_>) -> Result<PrintJob, rusqlite::Error> {
    let id_str: String = row.get(0)?;
    let printer_id_raw: String = row.get(1)?;
    let document_url: String = row.get(2)?;
    let status_str: String = row.get(3)?;
    let retry_count: i64 = row.get(4)?;
    let created_at: i64 = row.get(5)?;
    let completed_at: Option<i64> = row.get(6)?;
    let error_message: Option<String> = row.get(7)?;
    let output_path: Option<String> = row.get(8)?;
    let settings_json: Option<String> = row.get(9)?; // NEW

    // ... parse id, status như cũ ...

    // NEW: deserialize snapshot, fallback default nếu NULL hoặc lỗi parse
    let settings = settings_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    Ok(PrintJob::reconstruct(
        id,
        status,
        retry_count as u32,
        document_url,
        PrinterId::new(printer_id_raw),
        created_at,
        completed_at,
        error_message,
        output_path,
        settings, // thay cho PrintJobSettings::default()
    ))
}
```

### Bước 3.5 — Khôi phục snapshot khi `pop()` khỏi hàng đợi

File: `src-tauri/src/infrastructure/persistence/job_queue_broker.rs`, hàm `pop()`.

Đây là đường đi thực tế của worker (worker gọi `queue_manager.pop()`), nên **bắt buộc** sửa, nếu không settings vẫn bị default ngay tại điểm in.

Sửa câu SELECT (hiện: `id, printer_name, document_url, retry_count, output_path`) thêm `settings_json`:

```rust
let mut stmt = tx
    .prepare(
        "SELECT id, printer_name, document_url, retry_count, output_path, settings_json
         FROM print_jobs
         WHERE status = ?1
           AND (scheduled_at IS NULL OR scheduled_at <= ?2)
         ORDER BY created_at ASC
         LIMIT 1",
    )
    .map_err(|e| QueueError::RepositoryError(e.to_string()))?;
```

Trong closure `query_row` (hiện dựng job với `PrintJobSettings::default()` ở dòng 138):

```rust
let result = stmt.query_row(
    rusqlite::params![PrintStatus::Queued.to_db_string(), now],
    |row| {
        let id_str: String = row.get(0)?;
        let printer_id_raw: String = row.get(1)?;
        let document_url: String = row.get(2)?;
        let retry_count: i64 = row.get(3)?;
        let output_path: Option<String> = row.get(4)?;
        let settings_json: Option<String> = row.get(5)?; // NEW

        let id: PrintJobId = id_str.parse().map_err(|e: uuid::Error| {
            rusqlite::Error::InvalidColumnType(0, e.to_string(), rusqlite::types::Type::Text)
        })?;

        // NEW: khôi phục snapshot, fallback default
        let settings = settings_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();

        Ok(PrintJob::reconstruct(
            id,
            PrintStatus::Pending,
            retry_count as u32,
            document_url,
            PrinterId::new(printer_id_raw),
            0,
            None,
            None,
            output_path,
            settings, // thay cho PrintJobSettings::default()
        ))
    },
);
```

### Bước 4 — Lúc in dùng `job.settings` thay vì cấu hình hiện tại

File: `src-tauri/src/application/use_cases/process_print_job.rs`, hàm `render_or_save()` (nhánh `else`, dòng 106-119).

Thay:

```rust
let settings: PrintJobSettings = self
    .config_provider
    .load_print_config()?
    .as_ref()
    .map(PrintJobSettings::from)
    .unwrap_or_default();

self.print_service
    .print(pdf_path_str, job.printer_id().as_str(), &settings)?;
```

bằng:

```rust
// Dùng snapshot cấu hình đã đóng băng lúc tạo job, KHÔNG nạp lại cấu hình hiện tại,
// để job giữ đúng khổ giấy/hướng/màu... của thời điểm tạo.
self.print_service
    .print(pdf_path_str, job.printer_id().as_str(), &job.settings)?;
```

Đây là thay đổi **hành vi cốt lõi**: bỏ phụ thuộc vào cấu hình runtime khi in.

### Bước 5 — (Tùy chọn) Dọn dẹp `config_provider` không còn dùng

Sau Bước 4, `ProcessPrintJobUseCase` không còn dùng `config_provider`. Có thể gỡ để sạch:

- File `process_print_job.rs`: bỏ field `config_provider`, bỏ tham số trong `new()`, bỏ `use ... ConfigPort`.
- Nơi khởi tạo `ProcessPrintJobUseCase` (tìm trong `src-tauri/src/lib.rs` hoặc bootstrap DI): bỏ đối số truyền vào.

Nếu muốn giảm rủi ro review, có thể **giữ nguyên** field (chỉ thành unused) và xử lý sau. Khuyến nghị gỡ để tránh warning + hiểu nhầm.

---

## 5. (Khuyến nghị đi kèm) Bước 6 — Chống treo im lặng khi đổi máy in

Độc lập với snapshot, nhưng liên quan trực tiếp tới kịch bản "đổi máy in giữa chừng". Nếu máy in cũ bị **gỡ kết nối thật**, lời gọi Win32 GDI trong `printer_api/windows.rs` (`CreateDCW`/`StartDocW`/`StretchDIBits`) có thể **panic**, làm chết worker thread → các phiếu còn lại kẹt `QUEUED` vĩnh viễn, không phát `PrintJobFailed`, web treo ở `done:false`.

File: `src-tauri/src/infrastructure/worker/queue_worker.rs`, trong `process_loop`, thay `let result = process_use_case.execute(job);` bằng:

```rust
// Cô lập panic từng job: một job panic (vd. device context không hợp lệ sau khi
// đổi máy in) sẽ được chuyển thành lỗi và xử lý qua failure_handler, thay vì làm
// chết worker và treo cả batch.
let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    process_use_case.execute(job)
})) {
    Ok(r) => r,
    Err(panic_payload) => {
        let detail = panic_payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| panic_payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".to_string());
        Err(Error::Operation(format!(
            "Tiến trình in gặp lỗi nghiêm trọng và job này đã bị dừng: {}",
            detail
        )))
    }
};
```

Cần thêm `use crate::application::errors::Error;` vào đầu file. Nhánh `if let Err(e) = result { ... }` sẵn có sẽ reload job và gọi `failure_handler.handle(e, ...)` → đánh dấu `FAILED` + phát `PrintJobFailed`. Khả thi vì project đang dùng `panic = "unwind"` (không phải `abort`).

Lưu ý biên: nếu panic xảy ra cực sớm (trước khi `job.queue()` kịp persist QUEUED), reload có thể thấy status `PENDING` — mà `PENDING → FAILED` không hợp lệ (`print_status.rs`), khi đó `failure_handler` chỉ log lỗi. Trường hợp này hiếm; nếu muốn xử lý triệt để cần thêm cơ chế recover job mồ côi (ngoài phạm vi plan này).

---

## 6. Ảnh hưởng & rủi ro

- **Tương thích ngược**: cột mới nullable, row cũ fallback default → không phá dữ liệu.
- **Không đổi API/contract** với web app (`document_url`, các endpoint giữ nguyên).
- **Migration**: chạy tự động qua `rusqlite_migration`; idempotent.
- **Phạm vi file chạm**: `migrations.rs`, `print_job_repository.rs`, `job_queue_broker.rs`, `process_print_job.rs` (+ tùy chọn `lib.rs`/bootstrap, `queue_worker.rs`).

## 7. Test đề xuất

1. **Migration**: thêm test kiểm tra cột `settings_json` tồn tại sau `run_migrations` (theo mẫu `test_migration_5_adds_scheduled_at_column`).
2. **Repository round-trip**: `save()` một job có settings không mặc định (vd. `paper_size` khác A4, `orientation = "LANDSCAPE"`), rồi `find_by_id()` và assert settings khớp.
3. **Queue pop round-trip**: insert job với `settings_json`, `pop()`, assert `job.settings` khớp (không phải default).
4. **Fallback**: insert job với `settings_json = NULL`, đọc ra phải bằng `PrintJobSettings::default()`.
5. **Hành vi in**: (integration/manual) tạo job với cấu hình A → đổi cấu hình sang B → xác nhận job cũ vẫn in theo A.
6. **(Bước 6)** Test worker: giả lập `process_use_case` panic → worker vẫn sống, job được đánh dấu FAILED, job kế tiếp vẫn được xử lý.

## 8. Thứ tự thực hiện đề xuất

1. Bước 1 (migration) → 2 (save) → 3 & 3.5 (đọc) → 4 (in theo snapshot) → 5 (dọn dẹp).
2. Bước 6 (chống treo) có thể làm cùng lúc hoặc tách PR riêng — quyết định khi review.
