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
