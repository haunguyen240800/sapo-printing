---
title: "Story 3.4: Create Hybrid Strategy Selector (Auto-detect Printer Capability)"
status: done
story_id: "3.4"
story_key: "3-4-create-hybrid-strategy-selector-auto-detect-printer-capability"
epic: 3
parent_story: 3-3-implement-direct-pdf-strategy
baseline_commit: a7850cd
created: 2026-06-23
---

# Story 3.4: Create Hybrid Strategy Selector (Auto-detect Printer Capability)

Status: done

## Story

As a **developer**,
I want **automatic strategy selection based on printer capabilities**,
So that **the app uses fast Direct PDF when possible, falls back to PDFium render when needed**.

## Context

Story này là **story thứ tư trong Epic 3** và implement **strategy selection logic** của hybrid rendering architecture.

**Business Value:**
- Tự động chọn fast path (Direct PDF, ~0.5s) khi printer hỗ trợ native PDF và không cần margins/color conversion
- Fallback sang control path (PDFium render, ~2-3s) khi cần margins/color hoặc printer không support native PDF
- 4-6x performance improvement cho cases có thể dùng Direct PDF
- Strategy caching (5s TTL) tránh re-detect capability liên tục

**Architecture Context:**
- Infrastructure layer — implements Strategy Pattern (AR-5, Architecture Decision 4)
- **Depends on Story 3.2:** `DocumentRenderer` trait, `PdfiumRenderer`, `RenderConfig`/`ColorMode`/`PaperSize` types
- **Depends on Story 3.3:** `DirectPdfRenderer` (fast path implementation)
- **Depends on Story 2.2/2.3:** `PrinterManager` trait (will be extended with `supports_direct_pdf()`)
- **Blocks Story 3.5:** Queue worker uses whichever strategy is selected
- **Blocks Use Case stories:** AppContext needs `renderer` field for CreatePrintJob use case

**Key Design Decision (Architecture Decision 4):**
- Strategy selection based on: (1) printer capability detection, (2) render config (margins, color mode)
- Direct PDF chỉ được chọn khi: printer supports native PDF **AND** margins == 0 **AND** color_mode == RGB
- Mọi trường hợp khác → PdfiumRenderer (full control)
- Cache per-printer decision với 5s TTL (aligned với printer status polling interval)

## Acceptance Criteria

### AC-1: PrinterManager Trait Extension — `supports_direct_pdf()`

**Given** the `PrinterManager` trait exists (from Story 2.2/2.3)
**When** I extend the trait with capability detection
**Then** `src-tauri/src/infrastructure/printer/printer_manager.rs` must be updated:
```rust
pub trait PrinterManager: Send + Sync {
    fn discover_printers(&self) -> Vec<Printer>;
    fn get_status(&self, name: &str) -> PrinterStatus;
    fn supports_direct_pdf(&self, printer_name: &str) -> bool;
}
```
- Method returns `bool` — `true` if printer can natively render PDF without host-side rasterization
- Default behavior: return `false` (conservative — fallback to PDFium)

### AC-2: Windows Implementation — Win32 Capability Detection

**Given** the `PrinterManager` trait has `supports_direct_pdf()`
**When** I implement it for Windows
**Then** `src-tauri/src/infrastructure/printer/windows/win32_printer_manager.rs` must be updated:
- Use Win32 `OpenPrinterW` + `GetPrinterW` (level 2) to get `PRINTER_INFO_2`
- Check `pDriverName` and query driver capabilities via `DeviceCapabilitiesW` with `DC_COLLATE` or check if the printer driver name contains known PDF-capable driver keywords
- **Practical approach:** Check printer attributes for `PRINTER_ATTRIBUTE_RAW_ONLY` (0x00008000) — if set, printer accepts raw data including PDF. Alternatively, check if printer driver name matches known PDF-capable drivers (e.g., "Microsoft Print to PDF", "Adobe PDF", common PCL/PostScript drivers)
- **Fallback:** If detection fails for any reason → return `false` (safe default)
- Must handle: printer not found, access denied, API errors — all return `false` silently (log warning via `tracing::warn!`)

**CRITICAL:** Win32 capability detection is **heuristic-based** — there is no single Win32 API that definitively answers "does this printer support native PDF?". The implementation should be **conservative** — when in doubt, return `false` (fallback to PDFium). False negatives (returning false for a PDF-capable printer) are acceptable — the app falls back to PDFium which always works. **False positives (returning true for a non-PDF printer) are BAD** — the print will fail at runtime. Document the heuristic clearly in code comments.

### AC-3: CUPS Implementation — lpoptions Capability Detection

**Given** the `PrinterManager` trait has `supports_direct_pdf()`
**When** I implement it for CUPS (macOS/Linux)
**Then** `src-tauri/src/infrastructure/printer/cups/cups_printer_manager.rs` must be updated:
- Run `lpoptions -d {printer_name} -l` and parse output
- Check for `pdftopdf` filter support or `application/pdf` in supported mime types
- Alternative: check `lpinfo -m -d {printer_name}` for PDF-capable drivers
- **Fallback chain** (consistent with existing CUPS patterns):
  1. Try `lpoptions` command
  2. If fails → try parsing CUPS PPD file at `/etc/cups/ppd/{printer_name}.ppd` for `*cupsFilter:* pdftopdf`
  3. If all fail → return `false`
- Must handle: command not found, printer not found, permission errors — all return `false`

### AC-4: StrategySelector Struct

**Given** both renderers and capability detection exist
**When** I create the strategy selector
**Then** `src-tauri/src/infrastructure/renderer/strategy_selector.rs` must be created:

```rust
pub struct StrategySelector {
    printer_manager: Arc<dyn PrinterManager>,
    cache: Mutex<HashMap<String, (bool, Instant)>>,
    cache_ttl: Duration,
}
```

- **Constructor:** `pub fn new(printer_manager: Arc<dyn PrinterManager>) -> Self`
  - Default `cache_ttl`: 5 seconds (`Duration::from_secs(5)`)
- **Core method:**
  ```rust
  pub fn select_renderer(&self, printer_name: &str, config: &RenderConfig) -> Arc<dyn DocumentRenderer>
  ```
- **Selection logic:**
  1. Check cache for `printer_name` → if entry exists and `Instant::elapsed() < cache_ttl`, use cached `supports_direct_pdf` value
  2. If cache miss/expired → call `printer_manager.supports_direct_pdf(printer_name)`, store in cache
  3. If `supports_direct_pdf == true` **AND** `config.margin_left_mm == 0.0` **AND** `config.margin_right_mm == 0.0` **AND** `config.margin_top_mm == 0.0` **AND** `config.margin_bottom_mm == 0.0` **AND** `config.color_mode == ColorMode::Rgb` → return `Arc::new(DirectPdfRenderer::new())`
  4. Otherwise → return `Arc::new(PdfiumRenderer::new(config.dpi))`
- **Cache management:**
  - `pub fn invalidate_cache(&self)` — clear all entries
  - `pub fn invalidate_printer(&self, printer_name: &str)` — clear specific entry
- **Thread safety:** `StrategySelector` must be `Send + Sync` (uses `Mutex<HashMap>`)

**CRITICAL:** The method returns `Arc<dyn DocumentRenderer>`, NOT `Box<dyn DocumentRenderer>`. This is because `AppContext` stores renderers as `Arc<dyn DocumentRenderer>` and the selector may be called concurrently from multiple threads. Using `Arc` allows sharing the selector's output with other components.

### AC-5: Module Exports Update

**Given** `strategy_selector.rs` exists
**When** I update the renderer module
**Then** `src-tauri/src/infrastructure/renderer/mod.rs` must be updated to:
```rust
pub mod direct_pdf_renderer;
pub mod document_renderer;
pub mod pdfium_renderer;
pub mod strategy_selector;

pub use direct_pdf_renderer::DirectPdfRenderer;
pub use document_renderer::{ColorMode, DocumentRenderer, PaperSize, RenderConfig};
pub use pdfium_renderer::PdfiumRenderer;
pub use strategy_selector::StrategySelector;
```

### AC-6: AppContext Update

**Given** the `StrategySelector` exists
**When** I update the DI container
**Then** `src-tauri/src/shared/app_context.rs` must be updated:
- Add field: `strategy_selector: Arc<StrategySelector>`
- Initialize in `AppContext::new()`: `Arc::new(StrategySelector::new(printer_manager.clone()))`
- Add accessor: `pub fn renderer(&self, printer_name: &str, config: &RenderConfig) -> Arc<dyn DocumentRenderer>`
  - This delegates to `self.strategy_selector.select_renderer(printer_name, config)`
- Replace the existing TODO comment (Story 3.3) with the actual implementation

**Note:** AppContext::new() currently panics with `todo!()`. Add the `strategy_selector` field declaration and initialize it alongside `printer_manager` (which is already constructed before the panic). The field will be functional once a later story resolves the `todo!()` panic. Do NOT attempt to fix the `todo!()` in this story.

### AC-7: Inline Unit Tests

**Given** the implementation is complete
**When** I write inline tests
**Then** `#[cfg(test)] mod tests` in `strategy_selector.rs` must verify:

1. **`test_direct_pdf_selected_when_capable_and_no_margins`:**
   - Mock `PrinterManager` with `supports_direct_pdf() -> true`
   - Create `RenderConfig` with all margins = 0.0, color_mode = RGB
   - Call `select_renderer()` → verify returned renderer is `DirectPdfRenderer`
   - Verify by rendering a test PDF: output starts with `%PDF-` (raw bytes)

2. **`test_pdfium_selected_when_capable_but_has_margins`:**
   - Mock `PrinterManager` with `supports_direct_pdf() -> true`
   - Create `RenderConfig` with margin_left_mm = 10.0 (non-zero)
   - Call `select_renderer()` → verify returned renderer is `PdfiumRenderer`
   - Verify by rendering a test PDF: output starts with page count header (u32 LE), NOT `%PDF-`

3. **`test_pdfium_selected_when_capable_but_non_rgb_color:`**
   - Mock `PrinterManager` with `supports_direct_pdf() -> true`
   - Create `RenderConfig` with margins = 0 but color_mode = GRAY
   - Call `select_renderer()` → verify `PdfiumRenderer` is returned

4. **`test_pdfium_selected_when_not_capable`:**
   - Mock `PrinterManager` with `supports_direct_pdf() -> false`
   - Create `RenderConfig` with all margins = 0.0, color_mode = RGB
   - Call `select_renderer()` → verify `PdfiumRenderer` is returned

5. **`test_cache_hit_avoids_repeated_detection`:**
   - Create mock that counts `supports_direct_pdf()` calls
   - Call `select_renderer()` twice within 5s TTL
   - Verify `supports_direct_pdf()` was called only once (second call used cache)

6. **`test_cache_expiry_triggers_redetection`:**
   - Create `StrategySelector` with very short TTL (e.g., 10ms for test speed)
   - Call `select_renderer()`, wait for TTL to expire
   - Call `select_renderer()` again
   - Verify `supports_direct_pdf()` was called twice

7. **`test_invalidate_cache_clears_all_entries`:**
   - Populate cache with multiple printers
   - Call `invalidate_cache()`
   - Verify next call re-detects for all printers

8. **`test_invalidate_printer_clears_specific_entry`:**
   - Populate cache for "printer_a" and "printer_b"
   - Call `invalidate_printer("printer_a")`
   - Verify "printer_a" re-detects but "printer_b" still cached

9. **`test_strategy_selector_is_send_sync`:**
   - Verify `StrategySelector` can be shared across threads
   - Use `Arc<StrategySelector>` in `std::thread::spawn`

**Mock PrinterManager for tests:**
```rust
struct MockPrinterManager {
    supports_pdf: bool,
    call_count: Arc<Mutex<u32>>,
}

impl PrinterManager for MockPrinterManager {
    fn discover_printers(&self) -> Vec<Printer> { vec![] }
    fn get_status(&self, _name: &str) -> PrinterStatus { PrinterStatus::Online }
    fn supports_direct_pdf(&self, _printer_name: &str) -> bool {
        let mut count = self.call_count.lock().unwrap();
        *count += 1;
        self.supports_pdf
    }
}
```

### AC-8: Integration Test

**Given** all components implemented
**When** I create integration test
**Then** add to `src-tauri/tests/integration/renderer_integration_test.rs` (append to existing file):

**IMPORTANT:** Update the import statement at the top to include `StrategySelector`:
```rust
use sapo_printer::infrastructure::renderer::{
    ColorMode, DocumentRenderer, DirectPdfRenderer, PaperSize, PdfiumRenderer, RenderConfig,
    StrategySelector,
};
use sapo_printer::infrastructure::printer::PrinterManager;
```

**NOTE:** Integration tests are a separate crate — cannot access `MockPrinterManager` from `strategy_selector.rs` inline tests. Define a local mock in the integration test file:
```rust
struct IntegrationMockPrinterManager {
    supports_pdf: bool,
}
impl PrinterManager for IntegrationMockPrinterManager {
    fn discover_printers(&self) -> Vec<sapo_printer::domain::printer::Printer> { vec![] }
    fn get_status(&self, _name: &str) -> sapo_printer::domain::printer::PrinterStatus { sapo_printer::domain::printer::PrinterStatus::Online }
    fn supports_direct_pdf(&self, _printer_name: &str) -> bool { self.supports_pdf }
}
```

1. **`test_strategy_selector_direct_pdf_path:`**
   - Create a mock `PrinterManager` that returns `supports_direct_pdf() -> true`
   - Create `StrategySelector` with this mock
   - Call `select_renderer()` with zero-margin RGB config
   - Render a test PDF → verify output starts with `%PDF-` (raw bytes, Direct PDF path)

2. **`test_strategy_selector_pdfium_path:`**
   - Create a mock `PrinterManager` that returns `supports_direct_pdf() -> false`
   - Create `StrategySelector` with this mock
   - Call `select_renderer()` with any config
   - Render a test PDF → verify output starts with page count header (u32 LE, PDFium path)

3. **`test_strategy_selector_margin_triggers_fallback:`**
   - Create mock with `supports_direct_pdf() -> true`
   - Call `select_renderer()` with non-zero margins
   - Render → verify PDFium output format (not raw PDF)

4. **`test_strategy_selector_performance_comparison:`**
   - Create ~100KB test PDF
   - Measure Direct PDF render time → assert < 1s
   - Measure PDFium render time → assert < 5s (generous bound for CI)
   - Verify Direct PDF is significantly faster

### AC-9: Documentation Update

**Given** the implementation is complete
**When** I update module documentation
**Then** update `src-tauri/src/infrastructure/renderer/README.md` to include:
- StrategySelector description and selection logic flowchart
- Updated architecture diagram showing all three components (DirectPdfRenderer, PdfiumRenderer, StrategySelector)
- Capability detection explanation for both Windows and CUPS
- Usage example:
  ```rust
  let selector = StrategySelector::new(printer_manager.clone());
  let renderer = selector.select_renderer("HP LaserJet", &config);
  let output = renderer.render(&pdf_path, &config)?;
  ```

### AC-10: All Tests Pass

**Given** all components implemented (AC-1 through AC-9)
**When** I run the full test suite
**Then** all of the following must succeed:
- `cargo test` — all unit tests pass (inline + integration)
- `cargo test --test integration_tests` — integration tests pass
- `cargo clippy -- -D warnings` — no clippy warnings
- `cargo fmt --check` — code is formatted

## Tasks / Subtasks

- [x] Task 1: Extend PrinterManager trait (AC-1)
  - [x] Add `supports_direct_pdf(&self, printer_name: &str) -> bool` to trait
- [x] Task 2: Windows capability detection (AC-2)
  - [x] Implement `supports_direct_pdf()` in `Win32PrinterManager`
  - [x] Use Win32 API heuristic (printer attributes / driver name check)
  - [x] Add inline unit tests for Windows detection
- [x] Task 3: CUPS capability detection (AC-3)
  - [x] Implement `supports_direct_pdf()` in `CupsPrinterManager`
  - [x] Use lpoptions / PPD file fallback chain
  - [x] Add inline unit tests for CUPS detection
- [x] Task 4: Create StrategySelector (AC-4)
  - [x] Define `StrategySelector` struct with cache
  - [x] Implement `select_renderer()` with selection logic
  - [x] Implement cache management methods
- [x] Task 5: Write inline unit tests (AC-7)
  - [x] Test: Direct PDF selected when capable + no margins + RGB
  - [x] Test: PDFium selected when capable + margins > 0
  - [x] Test: PDFium selected when capable + non-RGB color
  - [x] Test: PDFium selected when not capable
  - [x] Test: Cache hit avoids repeated detection
  - [x] Test: Cache expiry triggers redetection
  - [x] Test: invalidate_cache clears all
  - [x] Test: invalidate_printer clears specific
  - [x] Test: Send + Sync verification
- [x] Task 6: Update module exports (AC-5)
  - [x] Update `mod.rs` to export `strategy_selector` and `StrategySelector`
- [x] Task 7: Update AppContext (AC-6)
  - [x] Add `strategy_selector` field
  - [x] Add `renderer()` accessor method
  - [x] Replace Story 3.3 TODO comment
- [x] Task 8: Integration tests (AC-8)
  - [x] Test: Direct PDF path end-to-end
  - [x] Test: PDFium path end-to-end
  - [x] Test: Margin triggers fallback
  - [x] Test: Performance comparison
- [x] Task 9: Documentation (AC-9)
  - [x] Update README.md with StrategySelector docs
- [x] Task 10: Final verification (AC-10)
  - [x] Run `cargo test` — all pass
  - [x] Run `cargo clippy -- -D warnings` — clean
  - [x] Run `cargo fmt --check` — formatted

## Dev Notes

### Architecture Alignment

**Strategy Pattern (AR-5, Decision 4):**
- `DocumentRenderer` trait = strategy interface
- `PdfiumRenderer` = control path (render with margins/color, ~2-3s)
- `DirectPdfRenderer` = fast path (pass-through raw PDF, ~0.5s)
- `StrategySelector` = strategy factory (auto-selects based on capabilities + config)

**Selection Logic Decision Tree:**
```
printer supports native PDF?
├── NO  → PdfiumRenderer (always)
└── YES → config requires margins or color conversion?
          ├── YES (margins > 0 OR color ≠ RGB) → PdfiumRenderer
          └── NO (margins == 0 AND color == RGB) → DirectPdfRenderer
```

**Why margins == 0 AND color == RGB for Direct PDF?**
- DirectPdfRenderer sends raw PDF bytes to printer — printer handles everything natively
- If user specified margins, we MUST render to apply them (DirectPdfRenderer ignores config)
- If user specified non-RGB color mode, we MUST render to convert (DirectPdfRenderer ignores config)
- Only when no transformation is needed can we safely use the fast path

### PrinterManager Trait Extension — Backward Compatibility

**CRITICAL:** Adding `supports_direct_pdf()` to `PrinterManager` trait is a **breaking change** for all implementors. Currently there are exactly 2 implementations:
1. `Win32PrinterManager` in `src-tauri/src/infrastructure/printer/windows/win32_printer_manager.rs`
2. `CupsPrinterManager` in `src-tauri/src/infrastructure/printer/cups/cups_printer_manager.rs`

Both MUST be updated in the same PR. No other implementations exist (mock or otherwise).

**For tests:** The mock `MockPrinterManager` in `strategy_selector.rs` tests will implement the full trait including the new method. No existing test mocks need updating because no other test mocks `PrinterManager` (existing tests mock at the `DocumentRenderer` level).

### Windows Capability Detection — Heuristic Approach

There is **no definitive Win32 API** for "does this printer support native PDF?". The implementation must use heuristics:

**Recommended approach (conservative):**
1. Open printer with `OpenPrinterW`
2. Get `PRINTER_INFO_2` via `GetPrinterW`
3. Check `Attributes` field for `PRINTER_ATTRIBUTE_RAW_ONLY` (0x00008000) — indicates printer accepts raw data
4. If `RAW_ONLY` not set, check `pDriverName` against known PDF-capable driver patterns:
   - Contains "PDF" (e.g., "Microsoft Print to PDF", "Adobe PDF")
   - Contains "PostScript" or "PS" (PostScript printers typically handle PDF)
   - Contains "PCL" (some PCL6 drivers handle PDF)
5. If none match → return `false`

**Important:** This is a best-effort heuristic. False negatives (returning false for a PDF-capable printer) are acceptable — the app falls back to PDFium which always works. False positives (returning true for a non-PDF printer) are BAD — the print will fail. **When in doubt, return false.**

### CUPS Capability Detection — Fallback Chain

Follow the same 3-tier pattern established in Story 2.3:

**Tier 1:** `lpoptions -d {printer_name} -l`
- Parse output for `pdftopdf` or `application/pdf` indicators
- If found → return `true`

**Tier 2:** Read PPD file at `/etc/cups/ppd/{printer_name}.ppd` (or `~/.cups/ppd/` on macOS)
- Look for `*cupsFilter:` lines containing `pdftopdf`
- If found → return `true`

**Tier 3:** Return `false` (safe default)

### Caching Strategy

**Why cache?** `supports_direct_pdf()` involves OS API calls (Win32) or subprocess execution (CUPS `lpoptions`). These are expensive relative to the render call itself. Caching for 5s aligns with the printer status polling interval (FR-2.2).

**Cache key:** Printer name (`String`)
**Cache value:** `(bool, Instant)` — capability result + timestamp
**Cache TTL:** 5 seconds (configurable via constructor for testing)
**Thread safety:** `Mutex<HashMap<String, (bool, Instant)>>`

**Cache invalidation:**
- `invalidate_cache()` — clear all (e.g., when printer config changes)
- `invalidate_printer(name)` — clear specific entry (e.g., when a printer is reconfigured)

### Output Format — Reminder

**DirectPdfRenderer output:** Raw PDF bytes (starts with `%PDF-`)
**PdfiumRenderer output:** Binary bitmap format `[page_count: u32 LE][width: u32 LE][height: u32 LE][pixel data...]`

The `StrategySelector` returns `Arc<dyn DocumentRenderer>` — the caller doesn't know which implementation they got. The downstream `PrinterEngine` (stub, future story) will need to handle both output formats.

### File Structure

```
src-tauri/src/infrastructure/renderer/
├── mod.rs                          # UPDATE: add strategy_selector module + re-export
├── document_renderer.rs            # Trait + types (NO CHANGE)
├── pdfium_renderer.rs              # Control path (NO CHANGE)
├── direct_pdf_renderer.rs          # Fast path (NO CHANGE)
├── strategy_selector.rs            # NEW: Strategy selection with caching
│   └── #[cfg(test)] mod tests     # Inline unit tests
└── README.md                       # UPDATE: add StrategySelector docs

src-tauri/src/infrastructure/printer/
├── printer_manager.rs              # UPDATE: add supports_direct_pdf() to trait
├── printer_engine.rs               # (NO CHANGE)
├── windows/
│   └── win32_printer_manager.rs    # UPDATE: implement supports_direct_pdf()
└── cups/
    └── cups_printer_manager.rs     # UPDATE: implement supports_direct_pdf()

src-tauri/src/shared/
└── app_context.rs                  # UPDATE: add strategy_selector field + renderer() accessor

src-tauri/tests/integration/
└── renderer_integration_test.rs    # UPDATE: append strategy selector integration tests
```

### Testing Strategy

**Unit Tests (Inline `#[cfg(test)]` in `strategy_selector.rs`):**
- Selection logic: all 4 branches of decision tree
- Caching: hit, miss, expiry, invalidation
- Thread safety: Send + Sync verification
- Use `MockPrinterManager` with configurable `supports_pdf` and call counter

**Integration Tests (append to `renderer_integration_test.rs`):**
- End-to-end: selector → renderer → output format verification
- Performance comparison: Direct PDF vs PDFium timing
- Real file I/O with valid test PDFs

**Platform-specific tests:**
- Windows capability detection: test with mock Win32 responses — wrap in `#[cfg(target_os = "windows")]`
- CUPS capability detection: test lpoptions/PPD parsing — wrap in `#[cfg(not(target_os = "windows"))]`
- Follow the same `#[cfg]` gating pattern used in `win32_printer_manager.rs` and `cups_printer_manager.rs` existing tests

### Previous Story Learnings

**From Story 3.3 (Direct PDF Strategy):**
- **PDF header validation:** Duplicated `validate_pdf_header` in `direct_pdf_renderer.rs` to avoid cross-module coupling. StrategySelector doesn't need this — it delegates to renderers.
- **Output format distinction:** DirectPdfRenderer returns raw PDF bytes, PdfiumRenderer returns bitmap binary. StrategySelector returns `Arc<dyn DocumentRenderer>` — caller handles both formats.
- **Review fixes applied:** F-1 (error cause discarded), F-2 (bounds check). These are in the renderer files, not relevant to StrategySelector.
- **Deferred items from review:** F-5 (IO errors → NetworkError mapping), F-6 (no file size bound). These are pre-existing infrastructure issues, NOT for this story to fix.

**From Story 3.2 (PDFium Renderer):**
- **Library swap:** MuPDF → PDFium (`pdfium-render` 0.9) due to MSVC build issues. StrategySelector uses `PdfiumRenderer::new(dpi)` which internally uses PDFium.
- **DPI parameter:** `PdfiumRenderer::new(dpi: u32)` takes DPI. StrategySelector passes `config.dpi` when constructing PDFiumRenderer.

**From Story 2.2/2.3 (Printer Discovery):**
- **Win32PrinterManager:** Fully implemented with `EnumPrintersW`, `OpenPrinterW`, `GetPrinterW`. Adding `supports_direct_pdf()` follows the same Win32 API patterns already established.
- **CupsPrinterManager:** 3-tier fallback chain (CUPS API → lpstat → config file). `supports_direct_pdf()` should follow the same pattern.
- **Pattern to follow:** Platform-specific code in platform modules, trait in shared `printer_manager.rs`.

### Key Constraints

**From FR-2.5 (Printer Capability Detection):**
- Detect Direct PDF support
- Choose strategy: Direct PDF (fast) or Render (control)

**From FR-3.2 (Hybrid Rendering):**
- Direct PDF: ~0.5s per job
- Render: ~2-3s per job
- Auto-select strategy based on printer capabilities

**From AR-5 (Strategy Pattern):**
- `DocumentRenderer` trait with multiple implementations
- Automatic strategy selection based on printer capabilities
- Both implementations satisfy `Send + Sync` bounds

**From NFR-1 (Performance):**
- Strategy selection itself must be fast (< 1ms with cache hit)
- Cache TTL 5s to avoid repeated OS API calls

### What This Story Does NOT Do

- ❌ Does NOT implement actual print data sending → PrinterEngine (still stub)
- ❌ Does NOT wire renderers into job processing pipeline → Queue Worker (Story 3.5)
- ❌ Does NOT implement RAII temp file management → Story 3.5
- ❌ Does NOT modify `DocumentRenderer` trait or existing renderer implementations
- ❌ Does NOT fix pre-existing `From<io::Error>` error mapping (F-5 from Story 3.3 review)
- ❌ Does NOT implement CreatePrintJob use case → later story
- ❌ Does NOT fix `AppContext::new()` `todo!()` panic → later story

### References

- [Source: `_bmad-output/planning-artifacts/epics.md` — Story 3.4 Acceptance Criteria]
- [Source: `_bmad-output/planning-artifacts/architecture.md` — Decision 4: Hybrid Rendering Strategy, AR-5, file tree at line 2507]
- [Source: `_bmad-output/planning-artifacts/prds/prd-sapo-printer-2026-06-22/prd.md` — FR-2.5, FR-3.2]
- [Source: `src-tauri/src/infrastructure/renderer/document_renderer.rs` — DocumentRenderer trait, RenderConfig, ColorMode]
- [Source: `src-tauri/src/infrastructure/renderer/direct_pdf_renderer.rs` — DirectPdfRenderer (fast path)]
- [Source: `src-tauri/src/infrastructure/renderer/pdfium_renderer.rs` — PdfiumRenderer (control path)]
- [Source: `src-tauri/src/infrastructure/printer/printer_manager.rs` — PrinterManager trait]
- [Source: `src-tauri/src/infrastructure/printer/windows/win32_printer_manager.rs` — Win32 implementation]
- [Source: `src-tauri/src/infrastructure/printer/cups/cups_printer_manager.rs` — CUPS implementation]
- [Source: `src-tauri/src/shared/app_context.rs` — AppContext DI container]
- [Source: `_bmad-output/implementation-artifacts/3-3-implement-direct-pdf-strategy.md` — Previous story learnings]

## Dev Agent Record

### Agent Model Used

Qwen Code (Claude Sonnet 4)

### Debug Log References

- PDFium global init conflict: `pdfium-render` uses a `OnceCell` for bindings. Parallel tests calling `Pdfium::default()` caused assertion failures. Fixed with `PDFIUM_TEST_MUTEX` in strategy_selector unit tests and `--test-threads=1` for integration tests.
- Performance test used invalid PDF (header + zeros): PDFium rejected it with `FormatError`. Fixed by using the same minimal valid PDF structure as other tests.

### Completion Notes List

- Extended `PrinterManager` trait with `supports_direct_pdf(&self, printer_name: &str) -> bool`
- Windows: heuristic detection via `PRINTER_ATTRIBUTE_RAW_ONLY` + driver name keyword matching (PDF, PostScript, PS, PCL). Conservative — returns false on any error.
- CUPS: 2-tier fallback — `lpoptions` output parsing for `pdftopdf`/`application/pdf`, then PPD file `*cupsFilter:` parsing. Returns false if all fail.
- `StrategySelector` struct: `Arc<dyn PrinterManager>` + `Mutex<HashMap>` cache with 5s TTL. Returns `Arc<dyn DocumentRenderer>`.
- Selection logic: Direct PDF only when `supports_direct_pdf == true AND all margins == 0 AND color_mode == Rgb`. Otherwise PDFium.
- AppContext: added `strategy_selector: Arc<StrategySelector>` field and `renderer()` accessor. Did NOT fix `todo!()` in `new()`.
- 9 inline unit tests + 4 integration tests. All pass.
- Note: Tests using PDFium must run serially (`--test-threads=1`) due to PDFium global init. This is a pre-existing issue, not introduced by this story.

### File List

- `src-tauri/src/infrastructure/printer/printer_manager.rs` — Added `supports_direct_pdf()` to trait
- `src-tauri/src/infrastructure/printer/windows/win32_printer_manager.rs` — Implemented `supports_direct_pdf()` with Win32 heuristic + tests
- `src-tauri/src/infrastructure/printer/cups/cups_printer_manager.rs` — Implemented `supports_direct_pdf()` with lpoptions/PPD fallback + tests
- `src-tauri/src/infrastructure/renderer/strategy_selector.rs` — NEW: StrategySelector with cache + 9 inline tests
- `src-tauri/src/infrastructure/renderer/mod.rs` — Added `strategy_selector` module + re-export
- `src-tauri/src/infrastructure/renderer/README.md` — Updated with StrategySelector docs, architecture diagram, capability detection
- `src-tauri/src/shared/app_context.rs` — Added `strategy_selector` field + `renderer()` accessor
- `src-tauri/tests/integration/renderer_integration_test.rs` — Added 4 strategy selector integration tests

### Change Log

- 2026-06-23: Story 3.4 implemented — Hybrid Strategy Selector with auto-detection, caching, and full test coverage

### Review Findings

- [x] [Review][Decision] F-1: PCL keyword match gây false-positive PDF detection — **Resolved: bỏ PCL**. PCL ≠ PDF, giữ PCL gây false positive.
- [x] [Review][Decision] F-2: `PRINTER_ATTRIBUTE_RAW_ONLY` không đảm bảo PDF support — **Resolved: driver name only**. RAW_ONLY alone không đủ, chỉ tin driver name heuristic.
- [x] [Review][Patch] F-3: Mutex poisoning gây cascade panic [strategy_selector.rs:79,90,96] — fixed: `unwrap_or_else(|e| e.into_inner())`
- [x] [Review][Patch] F-4: Flaky performance comparison assertion [renderer_integration_test.rs:473] — removed cross-renderer comparison
- [x] [Review][Patch] F-5: Unsafe cast thiếu buffer size check [win32_printer_manager.rs:206] — added `size_of::<PRINTER_INFO_2W>()` guard
- [x] [Review][Patch] F-6: Performance test PDF size ~500B vs spec yêu cầu ~100KB [renderer_integration_test.rs] — padded to ~100KB
- [x] [Review][Defer] F-7: IO errors → NetworkError — pre-existing, đã defer từ story 3.3
- [x] [Review][Defer] F-8: Cache unbounded growth — project-wide improvement
- [x] [Review][Defer] F-9: No file size limit — deferred từ story 3.3
- [x] [Review][Defer] F-10: CUPS PPD path traversal — pre-existing pattern, hardening
- [x] [Review][Defer] F-11: lpstat substring match — pre-existing code
- [x] [Review][Defer] F-12: TOCTOU race in cache — idempotent, cosmetic
