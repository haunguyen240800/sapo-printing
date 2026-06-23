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
