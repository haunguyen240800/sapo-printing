# Deferred Work

## Deferred from: code review of 2-1-setup-sqlite-database-with-migrations-schemas (2026-06-22)

- Home dir fallback `"."` if USERPROFILE/HOME unset — `.sapo-printer` resolves to CWD. Cross-platform concern for Epic 4.
- No DOWN migrations / rollback path — relevant if auto-update ships a bad migration. Epic 4 concern.
- `color_mode` / `orientation` columns lack CHECK constraints — Story 2.4 (PrinterRepository) should enforce at domain layer.
- Boolean columns (`print_as_image`, `enable_buffer`, `is_default`) lack `CHECK(value IN (0,1))` — Story 2.4 concern.
- `DbPool::get()` panics on mutex poison instead of returning `Result` — acceptable tradeoff for now; revisit if threading model changes.
- Negative / NaN `mm` input to unit conversion functions returns wrong result silently — no domain validation layer yet; defer to use-site callers.


## Deferred from: code review of 2-2-implement-windows-printer-discovery-status-monitoring-win32-api (2026-06-23)

- `map_win32_status`: combined flags (e.g. `OFFLINE | ERROR`) không có test — hành vi đúng (Offline ưu tiên) nhưng chưa documented/tested
- `PrinterName::new("")` khi `pPrinterName` null — cần xác minh domain validation của `PrinterName` có reject empty string không (Story 1.3)


## Deferred from: code review of story 2-4 (2026-06-23)

- **created_at preservation unclear** — Code is correct (ON CONFLICT doesn't update created_at unless explicitly in SET clause), but structure is misleading. Low priority clarity fix.

- **No concurrent access tests** — Repository uses Arc<Mutex<Connection>> for thread-safe access but no tests verify concurrent save()/find_all() calls. Valid but not in AC-6 test list.

- **No NULL buffer_size_kb test** — buffer_size_kb saved as NULL but never queried or tested. Defer until field is actually used in reconstruction.

- **printer_name not unique but used as key** — find_by_name() queries by printer_name but schema's unique constraint is on device_id. Two printers with same name returns first match. Schema design issue from Story 2.1, requires migration to fix.

- **SQL name length validation** — Extremely long printer names could hit SQLite limits causing silent truncation. Should be enforced in domain layer (PrinterName value object), not repository.

- **Error message info leak** — Repository error messages include full rusqlite error text which could expose sensitive data. Generic security concern, rusqlite errors unlikely to leak secrets in this context.

- **Printer::new() failure unchecked** — Code assumes Printer::new() is infallible but signature not shown in diff. If it returns Result, should handle construction failure. Needs domain layer verification.

- **Git commit template syntax** — .claude/settings.local.json line 27 has unclosed single quote in commit pattern: `"Bash(git commit -m ' *)"`. Settings file change, not production code.

- **Error trait impl not shown** — Diff adds PrinterDomainError::RepositoryError variant but doesn't show std::error::Error trait impl. Likely already implemented in earlier story.

**Tech Debt (requires domain model changes)**:

- **Add device_id to domain model** — Currently using printer_name as device_id (UPSERT key). Proper fix: add device_id field to Printer aggregate, pass from Windows/CUPS backends. Requires rework of Stories 1.3, 2.2, 2.3.

- **Add config fields to domain model** — printer_configs table has paper_size, margins, color_mode but Printer aggregate doesn't expose these. Config persistence deferred to Story 2.5/2.6.

- **Implement is_default logic** — AC-2 requires "unset other printers when saving default", but Printer aggregate lacks is_default field. Requires domain extension for printer configuration.


## Deferred from: code review of 2-5-create-printer-configuration-ui-basic-settings-3-sections.md (2026-06-23)

- **Configuration persistence** — Config fields (paper_size, margins, orientation) deferred to Story 2.6 - architectural separation between Printer aggregate and configuration table
- **PrinterStatus no polling/refresh** — Status fetched once on mount, no real-time updates. [PrinterStatus.tsx:1629-1649] — Story 4.2 handles real-time status polling
- **Margins default to 0mm** — May clip on printers with 3-5mm unprintable border. [PrinterConfigForm.tsx:1199-1202] — Product decision, not correctness bug
- **Accessibility: missing aria-invalid** — Form inputs lack aria-invalid and aria-describedby for screen readers. [PrinterConfigForm.tsx:1290+] — Accessibility improvement, not blocking
- **Default printer logic relies on unset is_default** — Frontend logic won't work until backend fixed. [PrinterConfigForm.tsx:1222-1224] — Blocked by is_default flag patch


## Deferred from: code review of 2-6-implement-secret-management-for-device-tokens-os-keychain (2026-06-23)

- **Windows use-after-free potential** [windows_credential_manager.rs:51] — False positive: Rust ownership ensures vectors live until after CredWriteW call. Rust's lifetime rules prevent this issue.

- **UTF-8 panic on non-UTF8 legacy data** [windows_credential_manager.rs:95] — Pre-existing data issue: Error handled correctly with Err(SecretRetrieveError). Edge case for legacy migrations, document if needed.

- **macOS unsigned app prompt spam** [macos_keychain.rs] — Operational concern: Already documented in AC-3 requirement, runtime check not feasible. User must sign app to avoid repeated keychain prompts.

- **Linux error suggests gnome-keyring on wrong platform** [main.rs:216] — False positive: Conditional compilation ensures this is Linux-only code, impossible to trigger on other platforms.

- **HashMap OOM panic** [linux_secret_service.rs:72] — System-level failure: Rust standard library behavior, no graceful handling possible at application level.


## Deferred from: code review of 3-2-implement-mupdf-renderer-with-color-mode-support (2026-06-23)

- **Pixel-by-pixel conversion performance** — `convert_bitmap_to_color_mode` uses nested Rust loops per pixel. For A4 at 600 DPI ARGB: ~139M pixels. SIMD or image crate optimization deferred.
- **mm_to_pixels u32 overflow** — Custom paper >100,000mm at 1200 DPI overflows u32. Saturates to u32::MAX. Pre-existing in `unit_conversion.rs`.
- **Vec::with_capacity overflow on 32-bit targets** — `actual_w * render_h * bpp` can overflow usize. Only relevant for extreme paper sizes.
- **Floating-point precision in margin validation** — `margin_left + margin_right >= width` uses exact f64 comparison. Epsilon comparison would be more robust.


## Deferred from: code review of story 3-3-implement-direct-pdf-strategy (2026-06-23)

- **TOCTOU race between validate and read** — Spec explicitly defines two-step process (validate header then read file). Inherent to design, not actionable without spec change.
- **IO errors misclassified as NetworkError** — Pre-existing `From<io::Error>` impl in `infrastructure_error.rs` maps all IO errors to `NetworkError`. Not caused by this story.
- **No upper bound on file size** — `std::fs::read` allocates unbounded memory. Spec does not define maximum file size. Defer to queue worker story.
- **Test temp dirs not cleaned up on panic** — Common Rust test pattern, not specific to this change. Project-wide improvement.
- **Broken symlink → wrong error variant** — Same root cause as IO misclassification above.


## Deferred from: code review of story 3-5-implement-raii-temp-file-management-with-tiered-cleanup (2026-06-23)

- **Mid-session cleanup cho long-running sessions** — `startup_cleanup` chỉ chạy 1 lần khi start. App chạy nhiều ngày (kiosk/POS) → deferred .pdf files tích lũy không giới hạn. Defer sang queue worker hoặc monitoring story.
- **Subdirectory cleanup không recursive** [temp_file.rs:80] — `startup_cleanup` chỉ scan top-level entries. Nếu temp_dir chứa subdirectories, files bên trong không bao giờ bị cleanup. Hiện tại flat structure nên không issue, nhưng future change có thể tạo gap.
- **Không có protection chống 2 TempPdfFile cùng wrap 1 path** [temp_file.rs:21] — Nếu caller tạo 2 instances cùng path, first drop xóa file, second drop gets NotFound (silently ignored). Latent risk vì chưa có production callers, nhưng API không guard được.

## Deferred from: code review of story 3-4-create-hybrid-strategy-selector-auto-detect-printer-capability (2026-06-23)

- **Cache unbounded growth** — `StrategySelector` cache `HashMap<String, (bool, Instant)>` có TTL nhưng không có max size. Stale entries không bị evict. In print server environments với nhiều ephemeral network printers, đây là slow memory leak. Fix: thêm LRU eviction hoặc periodic cleanup.
- **CUPS PPD path traversal via printer name** — `ppd_has_pdf_filter()` constructs path `format!("/etc/cups/ppd/{}.ppd", printer_name)`. Printer name với `../` có thể read arbitrary files. Printer names thường từ CUPS API (không phải user input), nhưng defense-in-depth nên validate. Fix: reject names chứa `/`, `\`, `..`.
- **lpstat substring match** — `lpstat_get_status()` dùng `line.contains(name)` thay vì exact match. Printer "HP" match "HP_LaserJet". Pre-existing code, không thay đổi trong story này.
- **TOCTOU race in capability cache** — `check_capability()` drop lock giữa check và insert. Concurrent calls cho cùng printer sau TTL expiry sẽ gọi `supports_direct_pdf()` redundant. Result vẫn correct (idempotent), chỉ wasteful OS API calls.

## Deferred from: code review of 3-6-create-print-job-tables-event-store-schema (2026-06-23)

- `created_at`/`updated_at` không có DEFAULT hoặc trigger — pre-existing pattern từ MIGRATION_1/MIGRATION_2, client phải cung cấp đúng timestamp.
- AC-6 cargo test/build/clippy/fmt không verifiable từ diff — Tauri native build constraint đã biết từ các story trước.

## Deferred from: code review of story 3-6-implement-auto-retry-logic-with-exponential-backoff (2026-06-24)

- **Race condition on job reload from DB** [queue_worker.rs:1701-1704] — If concurrent workers modify job between `process_job` failure and `find_by_id`, loaded job may be stale. Unlikely in single-worker (Story 3.5) but will break with concurrent workers. Defer to Story 4.x (concurrent workers).
- **Race condition in requeue during shutdown** [queue_worker.rs:1920-1923] — `requeue()` may succeed but worker stops before processing, leaving job stuck in Queued state. Defer to Story 3.7 or 4.x (graceful shutdown improvements).
- **Mutex poison recovery unsafe** [queue_worker.rs:1282, 1639, 1656] — `unwrap_or_else(|p| p.into_inner())` recovers poisoned mutex without validating data consistency. If panic occurred mid-write, data may be corrupt. Defer - requires architectural decision on panic recovery strategy.
- **Test timing assertions flaky** [retry_integration_test.rs:4078-4082] — Fixed threshold `elapsed <= 45s` vulnerable to slow CI. May cause spurious failures. Defer to test infrastructure improvements (not story 3.6 scope).
- **No persist after retry() fails** [queue_worker.rs:1936-1952] — When `retry()` fails (MaxRetryExceeded), error is printed but job not persisted. Job already FAILED in memory, should persist to avoid orphaned state. Defer - minor consistency issue, low impact.
- **No validation for out-of-range retry_count** [retry_logic.rs:2920-2925] — `calculate_backoff_delay()` accepts u32 but only handles 0-2, falls through to `_ => 20` with comment "should never reach here". Defer - add debug_assert or explicit documentation (low priority).
- **No warning for suspicious retry_count in pop()** [sqlite_queue_manager.rs:69-96] — DB corruption could allow job with retry_count >= 3 in Queued state. Current behavior safe (fails permanently) but no warning logged. Defer - defensive programming, not critical.
- **String-based error classification cannot use typed is_retryable()** [queue_worker.rs:1897-1901] — `handle_job_failure()` receives `error: String` (formatted from InfrastructureError), cannot use typed `is_retryable(&InfrastructureError)`. Requires refactoring `process_job()` return type from `Result<(), String>` to `Result<(), InfrastructureError>`. Defer - architectural change affecting error propagation throughout worker pipeline.
- **Spurious PrintJobFailed events before retry** [queue_worker.rs:1887-1894] — Domain constraint: `job.retry()` requires job in FAILED state, so must call `job.fail()` first. External systems see "failed" event even when job will retry. Defer - requires domain model change to allow retry from non-FAILED states OR introduce "transient failure" vs "permanent failure" events.

## Deferred from: code review of story 3-5-implement-queue-worker-with-batch-processing (2026-06-24)

- **Failed jobs stuck in intermediate state** — When any pipeline step fails, process_job returns Err and the worker logs + continues. Job remains in last successful status (Queued/Downloaded/SubmittedToQueue/Printing) permanently. No PrintJobFailed event emitted. Spec defers to Story 3.6 (auto-retry with exponential backoff).
- **rendered_data memory pressure** — renderer.render() returns Vec<u8> holding full rendered bitmap (~26MB per A4 page at 300 DPI). Held in memory across multiple state transitions and DB writes until print completes. For large documents + slow printers, memory is pinned indefinitely. MVP acceptable with sequential single-threaded processing.
- **pop() error infinite retry with no backoff** — When queue_manager.pop() returns Err, worker logs, sleeps 500ms, retries indefinitely. No exponential backoff, no circuit breaker, no max retry count. Persistent DB issues cause endless stderr spam.
- **Integration tests thread::sleep polling flakiness** — Tests poll with fixed 500ms intervals and total budgets of 5s/10s. Under heavy CI load, OS scheduling delays may cause timeouts. No diagnostic output on failure.
- **AC-10 cargo clippy not clean** — Pre-existing clippy warnings in print_job_repository.rs and app_context.rs. Not caused by queue worker changes but blocks project-wide `cargo clippy -- -D warnings`.
- **start() after failed stop() inconsistent state** — If stop() fails (thread panicked), running flag is false but shared Arc references may point to poisoned/inconsistent state (e.g., SQLite Mutex). A subsequent start() succeeds but inherits broken state.

## Deferred from: code review of story 3-3-implement-createprintjobusecase-with-event-publishing (2026-06-24)

- **AC-4 Tauri Command Implementation Pattern Deviates from Spec** [src-tauri/src/interface/tauri/commands/print_job.rs, src-tauri/src/main.rs:2163-2172] — Spec shows `#[tauri::command]` on function in commands module, but implementation uses helper in lib crate + wrapper in main.rs. Auto-skill documents this is correct pattern for lib+bin crate split. Architectural improvement over spec.
- **AppContextState Location Differs from Spec Guidance** [src-tauri/src/lib.rs:2120-2140] — Spec indicates struct should be in main.rs but implementation moved to lib.rs as public struct. Required for crate visibility across lib+bin boundary per auto-skill pattern.

## Deferred from: code review of story 3-6-implement-auto-retry-logic (fresh review 2026-06-24)

- **AC-3 Deviation - Job marked FAILED before retry decision** [queue_worker.rs:489-493] — Domain constraint requires `job.retry()` to be called on FAILED job, so must call `job.fail()` first. Event stream shows false "failed" events for transient errors that successfully retry. Defer - requires domain architecture change to allow retry from non-FAILED states OR introduce separate "transient failure" vs "permanent failure" events.
- **Migration UPDATE may hang on table lock** [migrations.rs:34] — `UPDATE print_jobs SET scheduled_at = updated_at WHERE scheduled_at IS NULL` may block indefinitely if table locked during concurrent writes. No timeout or retry logic. Defer - pre-existing migration pattern used across all migrations, should be addressed project-wide.

## Deferred from: code review of story 3-7-implement-job-cancellation-use-case (2026-06-24)

- **Race condition on concurrent job mutations** [queue_worker.rs:451-463] — If concurrent workers modify job between process_job failure and find_by_id, loaded job may be stale. Story 3.5 issue, documented as "will break with concurrent workers". Defer to Story 4.x.
- **Mutex poison recovery unsafe** [queue_worker.rs:1282, 1639, 1656] — unwrap_or_else(|p| p.into_inner()) recovers poisoned mutex without validating data consistency. If panic occurred mid-write, data may be corrupt. Story 3.5 architectural decision needed.
- **Failed jobs orphaned in intermediate state** — When pipeline step fails, job remains in last successful status (Queued/Downloaded/Printing) permanently. Resolved by Story 3.6 auto-retry logic.
- **Requeue during shutdown leaves job stuck** [queue_worker.rs:1920-1923] — requeue() may succeed but worker stops before processing. Story 3.6 issue, requires graceful shutdown improvements.
- **pop() error infinite retry with no backoff** — When queue_manager.pop() returns Err, worker retries indefinitely with 500ms sleep. Story 3.5 issue, needs circuit breaker.
- **Retry failure not persisted** [queue_worker.rs:1936-1952] — When retry() fails (MaxRetryExceeded), job is FAILED in memory but not persisted. Story 3.6 consistency issue.
- **String-based error classification loses type safety** [queue_worker.rs:1897-1901] — handle_job_failure receives error: String, cannot use typed is_retryable(). Story 3.6 architectural change needed.
- **Spurious PrintJobFailed events before retry** [queue_worker.rs:1887-1894] — Domain constraint: job.retry() requires FAILED state, so job.fail() must be called first. External systems see false failures. Story 3.6 domain model change needed.
- **rendered_data memory pressure** — renderer.render() returns Vec<u8> (~26MB per A4 page) held in memory across state transitions. Story 3.5 acceptable for MVP with sequential processing.
- **Migration UPDATE may hang on table lock** [migrations.rs:180] — UPDATE print_jobs WHERE scheduled_at IS NULL has no timeout. Pre-existing migration pattern, should be addressed project-wide.
- **start() after failed stop() inherits broken state** — If stop() fails (thread panic), running flag is false but Arc refs may be poisoned. Story 3.5 issue.
- **No validation for out-of-range retry_count** [retry_logic.rs:1654-1660] — calculate_backoff_delay accepts u32 but only handles 0-2, falls through to _ => 20. Story 3.6 low priority defensive check.
- **No warning for suspicious retry_count in pop()** — DB corruption could allow retry_count >= 3 in Queued state. Story 3.6 defensive programming, behavior safe but no warning.
- **Test timing assertions flaky** [retry_integration_test.rs:4078-4082] — Fixed threshold elapsed <= 45s vulnerable to slow CI. Test infrastructure improvement needed.
- **Integration tests fixed sleep polling** — Tests poll with 500ms intervals and 5s/10s budgets. Heavy CI load may cause timeouts. Test infrastructure improvement needed.
- **requeue success but persist fails rollback** [queue_worker.rs:665-672] — If queue_manager.requeue succeeds but persist_and_publish fails, job requeued in DB but memory state FAILED. Story 3.6 rollback logic exists.
- **persist fails twice after MaxRetryExceeded** [queue_worker.rs:693-710] — When retry() returns MaxRetryExceeded and persist fails twice, failed job state may be lost. Story 3.6 retry logic present.

## Deferred from: code review of story 3-8-create-print-job-list-ui-with-filters (2026-06-24)

- **D-1: `execute_get_job_status` bypass use case layer** — Truy cập `job_repo.find_by_id()` trực tiếp thay vì qua use case. Architectural inconsistency với các command khác. Cần refactor khi có GetJobStatusUseCase.
- **D-2: Filter printer_name in-memory thay vì repository** — `result.retain()` filter printer_name sau khi load tất cả jobs. Performance concern khi có nhiều jobs. Cần repository method `find_by_printer_name()`.
- **D-3: PENDING và QUEUED cùng label "Đang chờ"** — User không phân biệt được 2 trạng thái trên UI. UX improvement, cần label khác nhau.
- **D-4: Race condition khi thay đổi filter nhanh** — Nhiều `loadJobs()` async chạy song song, response cũ có thể ghi đè response mới. Cần AbortController hoặc request cancellation.

## Deferred from: code review of 4-2-implement-status-polling-sync-2s-interval (2026-06-24)

- `updated_at` always `None` — dead field. `PrintJob` aggregate needs an `updated_at` field to support this DTO field. Track as future story.
- `status_to_string` duplicated between `job_dto.rs` and `job_status_dto.rs`. Follow same shared helper extraction pattern as `calculate_progress` in future refactor.
- `handle_get_status` instantiates `GetJobStatusUseCase` per-request. Matches existing pattern for `handle_cancel_job` and `handle_print_batch`. Consider use-case-level injection across all handlers in future refactor.
- `test_integration_rapid_polling_sequence` doesn't test state transitions. Adequate as stress test for happy path; state-transition-while-polling would require more complex test harness.

## Deferred from: code review of 4-3-setup-structured-logging-with-tracing-crate (2026-06-24)

- `read_dir` errors silently discarded in `cleanup_old_logs` — low risk since directory was just created.
- Symlink handling in cleanup — low practical risk on Windows.
- Regex recompilation on every `cleanup_old_logs` call — called once at startup, not a hot path.
- Malformed filename dates silently skipped — correct behavior for invalid dates.
- `&PathBuf` vs `&Path` parameter — idiomatic Rust style issue, not a correctness bug.
- Response truncation allocates new String in native messaging — minor CPU overhead per message.
- `chrono` and `regex` added as direct dependencies — reasonable implementation choice for cleanup logic.
- Existing tracing calls preservation not confirmed in `temp_file.rs`, `sqlite_queue_manager.rs`, `win32_printer_manager.rs` — files not in diff.
- `init_logging()` called at both `main()` and `run_native_messaging_mode()` — by design (idempotent).
- Invalid input logged at INFO level in `ListJobsUseCase` — minor noise concern.
- `job.status()` evaluated unconditionally before debug level check — not expensive currently.
- `chrono::and_hms_opt` may be deprecated in future chrono versions — maintenance concern.

## Deferred from: code review of story 4-5 (2026-06-25)

- **Mutex contention across 4 sequential queries** [collector.rs:collect_metrics] — Lock held across job, queue, printer, and performance queries including full-table scan of completed jobs. Blocks all other DB users during metrics collection. Performance concern at scale.
- **`fetch_completed_durations` unbounded memory growth** [collector.rs:fetch_completed_durations] — No LIMIT on query, loads all completed job durations into Vec, then clones for sorting. Memory grows linearly with total completed jobs over app lifetime.
- **Mutex poisoning unrecoverable** [collector.rs:collect_metrics] — If `queue_manager.queue_depth()` panics while Mutex is held, mutex becomes permanently poisoned. No recovery path. Pre-existing project-wide pattern from earlier stories.

## Deferred from: code review of story 4-4 (2026-06-25)

- **Mutex poisoning recovery** — `unwrap_or_else(|p| p.into_inner())` silently recovers from poisoned Mutex<Connection> in event_store.rs. Pre-existing pattern from earlier stories.
- **`save_all` identical timestamps** — 1-second granularity of `SystemTime::now().as_secs()` means all events in a batch share the same timestamp, reducing forensic fidelity. Design limitation.
- **`get_or_create_signing_key` re-entrancy hazard** — Takes `&self` while connection mutex is held. If future SecretManager implementation uses same DB, deadlock. Currently safe (OS credential store).
- **`SystemTime::now().unwrap()` theoretical panic** — Panics if system clock before Unix epoch. Pre-existing, not practical on modern OS.

