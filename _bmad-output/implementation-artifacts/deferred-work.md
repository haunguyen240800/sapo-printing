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
