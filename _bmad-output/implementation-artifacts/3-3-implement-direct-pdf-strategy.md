---
title: "Story 3.3: Implement Direct PDF Strategy"
status: done
story_id: "3.3"
story_key: "3-3-implement-direct-pdf-strategy"
epic: 3
parent_story: 3-2-implement-mupdf-renderer-with-color-mode-support
baseline_commit: 6cf96beab8230f12f86402d8d3ca5600d96b1c00
created: 2026-06-23
---

# Story 3.3: Implement Direct PDF Strategy

Status: review

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a **nhân viên kho** (warehouse staff),
I want **fast Direct PDF printing when the printer supports it**,
So that **jobs complete in ~0.5s instead of ~2-3s render time**.

## Context

Story này là **story thứ ba trong Epic 3** và implement **fast path** của hybrid rendering strategy.

**Business Value:**
- Direct PDF printing nhanh hơn 4-6x so với render path (~0.5s vs ~2-3s per job)
- Cho phép throughput cao hơn cho bulk printing (100+ đơn/phút)
- Giảm CPU usage vì không cần render PDF thành bitmap

**Architecture Context:**
- Infrastructure layer — implements `DocumentRenderer` trait (Strategy Pattern, AR-5)
- **Depends on Story 3.2:** `DocumentRenderer` trait, `PdfiumRenderer` (sibling implementation), `RenderConfig`/`ColorMode`/`PaperSize` types
- **Depends on Story 3.1:** `DocumentDownloader` trait, `InfrastructureError` types, temp file patterns
- **Blocks Story 3.4:** Hybrid strategy selector cần cả hai renderers để chọn strategy
- **Blocks Story 3.5:** Queue worker uses whichever strategy is selected

**Key Design Decision (Architecture Decision 4):**
- Direct PDF = pass-through, gửi raw PDF bytes trực tiếp đến printer
- Printer tự xử lý PDF natively — không render, không margin conversion, không color mode conversion
- Output là **raw PDF bytes** (KHÔNG phải bitmap format như PdfiumRenderer)
- PrinterEngine (stub hiện tại, sẽ implement ở story sau) chịu trách nhiệm gửi raw bytes đến printer

## Acceptance Criteria

### AC-1: DirectPdfRenderer Struct và Constructor

**Given** the `DocumentRenderer` trait exists (from Story 3.2)
**When** I create the Direct PDF strategy implementation
**Then** `src-tauri/src/infrastructure/renderer/direct_pdf_renderer.rs` must be created:
- Struct: `pub struct DirectPdfRenderer;` (unit struct — no state needed)
- Constructor: `pub fn new() -> Self`
- Implement `Default` trait: `impl Default for DirectPdfRenderer { fn default() -> Self { Self::new() } }`
- Implement `DocumentRenderer` trait:
  ```rust
  impl DocumentRenderer for DirectPdfRenderer {
      fn render(&self, path: &Path, config: &RenderConfig) -> Result<Vec<u8>, InfrastructureError> {
          // ...
      }
  }
  ```
- **Module-level documentation** explaining:
  - Purpose: Fast-path PDF printing — sends raw PDF bytes directly to printer
  - When to use: Printer supports native PDF rendering (capability detection in Story 3.4)
  - Trade-off: ~0.5s vs ~2-3s, but no margin/color control
  - Output format: Raw PDF bytes (NOT bitmap format like PdfiumRenderer)
  - Integration: Selected by StrategySelector (Story 3.4), used by QueueWorker (Story 3.5)

### AC-2: Render Implementation — Pass-through PDF Bytes

**Given** the `DirectPdfRenderer` struct exists
**When** `render()` is called with a valid PDF path
**Then** the implementation must:
1. **Validate PDF header:** Read first 5 bytes of file, verify starts with `%PDF-`
   - If validation fails → return `InfrastructureError::ValidationError`
   - Use same validation pattern as `ReqwestDownloader::validate_pdf_header()`
2. **Read entire file:** `std::fs::read(path)` to get raw bytes
3. **Return bytes directly:** No transformation, no rendering, no margin application, no color conversion
4. **Ignore config:** The `RenderConfig` parameter is accepted (trait contract) but intentionally unused — printer handles margins/color natively
5. **Error handling:**
   - Empty file (0 bytes) → `InfrastructureError::ValidationError` — check `std::fs::metadata(path)?.len() == 0` BEFORE reading header
   - File not found → `InfrastructureError::NetworkError` (via `From<std::io::Error>` catch-all)
   - Permission denied → `InfrastructureError::NetworkError` (via `From<std::io::Error>` catch-all)
   - Invalid PDF header → `InfrastructureError::ValidationError`
   - File < 5 bytes (too short for header) → `InfrastructureError::ValidationError`

   **CRITICAL:** Empty file check must come BEFORE `read_exact()`. Without it, `read_exact` on 0 bytes returns `UnexpectedEof` which maps to `NetworkError` via `From<std::io::Error>`, NOT `ValidationError`.

**And** execution time target: **< 1s** for typical invoice PDF (~100KB) per FR-3.2
**And** performance target: **~0.5s** per job per FR-3.2

### AC-3: Module Exports Update

**Given** `direct_pdf_renderer.rs` exists
**When** I update the renderer module
**Then** `src-tauri/src/infrastructure/renderer/mod.rs` must be updated to:
```rust
pub mod document_renderer;
pub mod pdfium_renderer;
pub mod direct_pdf_renderer;

pub use document_renderer::{ColorMode, DocumentRenderer, PaperSize, RenderConfig};
pub use pdfium_renderer::PdfiumRenderer;
pub use direct_pdf_renderer::DirectPdfRenderer;
```

### AC-4: Inline Unit Tests

**Given** the implementation is complete
**When** I write inline tests
**Then** `#[cfg(test)] mod tests` in `direct_pdf_renderer.rs` must verify:

1. **`test_direct_pdf_returns_raw_bytes_unchanged`:**
   - Create a minimal valid PDF file in temp dir
   - Call `render()` → verify output bytes are identical to input file bytes
   - Assert `output == file_content` (byte-for-byte match)

2. **`test_direct_pdf_preserves_pdf_header`:**
   - Create valid PDF, call `render()`
   - Verify output starts with `%PDF-` (first 5 bytes)

3. **`test_direct_pdf_invalid_file_returns_validation_error`:**
   - Create file with non-PDF content (`"not a pdf"`)
   - Call `render()` → assert `is_err()` and error is `ValidationError`

4. **`test_direct_pdf_missing_file_returns_error`:**
   - Call `render()` with non-existent path
   - Assert `is_err()`

5. **`test_direct_pdf_empty_file_returns_validation_error`:**
   - Create empty file (0 bytes)
   - Call `render()` → assert `is_err()` and error is `ValidationError`

6. **`test_direct_pdf_ignores_config`:**
   - Create valid PDF
   - Call `render()` with different configs (margins, color modes, paper sizes)
   - Verify output is identical regardless of config (config is ignored)

7. **`test_direct_pdf_performance`:**
   - Create a ~100KB PDF file. Simple approach — no need for valid PDF structure, just a valid header + padding:
     ```rust
     let dir = std::env::temp_dir().join("sapo_direct_pdf_perf");
     let _ = std::fs::create_dir_all(&dir);
     let path = dir.join("perf_100kb.pdf");
     let mut content = b"%PDF-1.4\n".to_vec();
     content.extend(vec![0u8; 100_000]); // pad to ~100KB
     std::fs::write(&path, &content).unwrap();
     ```
   - Measure execution time with `std::time::Instant`
   - Assert < 1 second
   - Cleanup temp dir after test

8. **`test_direct_pdf_trait_object_compatible`:**
   - Verify `DirectPdfRenderer` works as `Arc<dyn DocumentRenderer>`
   - Same pattern as Story 3.2

9. **`test_direct_pdf_default_constructor`:**
   - Verify `DirectPdfRenderer::default()` works
   - Verify `DirectPdfRenderer::new()` works

### AC-5: Integration Test

**Given** all components implemented
**When** I create integration test
**Then** add to `src-tauri/tests/integration/renderer_integration_test.rs` (append to existing file):

**IMPORTANT:** Update the import statement at the top of the file to include `DirectPdfRenderer`:
```rust
use sapo_printer::infrastructure::renderer::{
    ColorMode, DocumentRenderer, DirectPdfRenderer, PaperSize, PdfiumRenderer, RenderConfig,
};
```

1. **`test_direct_pdf_output_is_valid_pdf`:**
   - Create valid PDF, render with `DirectPdfRenderer`
   - Verify output starts with `%PDF-`
   - Verify output ends with `%%EOF`

2. **`test_direct_pdf_file_size_matches_input`:**
   - Create valid PDF with known size
   - Render with `DirectPdfRenderer`
   - Verify output size == input file size (no overhead)

3. **`test_direct_pdf_vs_pdfium_output_differs`:**
   - Create valid PDF
   - Render with both `DirectPdfRenderer` and `PdfiumRenderer`
   - Verify outputs are different (Direct PDF = raw bytes, PDFium = bitmap format)
   - Verify Direct PDF output starts with `%PDF-`
   - Verify PDFium output starts with page count header (u32 LE)

4. **`test_direct_pdf_multipage_preserves_all_pages`:**
   - Create multi-page PDF (2 pages)
   - Render with `DirectPdfRenderer`
   - Verify output contains full PDF (all pages preserved, not just first page)

### AC-6: AppContext TODO Comment

**Given** the DirectPdfRenderer implementation exists
**When** I prepare for future integration
**Then** update the TODO comment in `src-tauri/src/shared/app_context.rs`:
```rust
// TODO (Story 3.3): DirectPdfRenderer available for strategy selection
// Story 3.4 will add StrategySelector that chooses between:
//   - DirectPdfRenderer (fast path, ~0.5s, no margin/color control)
//   - PdfiumRenderer (control path, ~2-3s, full margin/color support)
// For now, neither is wired into AppContext — deferred to Use Case stories.
```

### AC-7: Documentation Update

**Given** the implementation is complete
**When** I update module documentation
**Then** update `src-tauri/src/infrastructure/renderer/README.md` to include:
- DirectPdfRenderer description and when to use it
- Comparison table: DirectPdfRenderer vs PdfiumRenderer
- Architecture diagram showing strategy pattern with both implementations
- Usage example:
  ```rust
  // Fast path — no rendering, raw PDF bytes
  let renderer = DirectPdfRenderer::new();
  let pdf_bytes = renderer.render(&pdf_path, &config)?;
  // pdf_bytes == raw PDF file content, ready to send to printer
  ```

### AC-8: All Tests Pass

**Given** all components implemented (AC-1 through AC-7)
**When** I run the full test suite
**Then** all of the following must succeed:
- `cargo test` — all unit tests pass (inline + integration)
- `cargo test --test integration_tests` — integration tests pass
- `cargo clippy -- -D warnings` — no clippy warnings
- `cargo fmt --check` — code is formatted

## Tasks / Subtasks

### Task 1: Create DirectPdfRenderer (AC-1, AC-2)
- [x] Create `direct_pdf_renderer.rs` trong `src-tauri/src/infrastructure/renderer/`
- [x] Define `DirectPdfRenderer` unit struct
- [x] Implement `new()` constructor và `Default` trait
- [x] Implement `DocumentRenderer` trait:
  - [x] Validate PDF header (first 5 bytes = `%PDF-`)
  - [x] Read entire file with `std::fs::read()`
  - [x] Return raw bytes (pass-through)
  - [x] Handle errors: file not found, invalid PDF, empty file
- [x] Add module-level documentation

### Task 2: Write Inline Unit Tests (AC-4)
- [x] Test: raw bytes unchanged
- [x] Test: PDF header preserved
- [x] Test: invalid file → ValidationError
- [x] Test: missing file → error
- [x] Test: empty file → ValidationError
- [x] Test: config ignored (same output for different configs)
- [x] Test: performance < 1s for ~100KB PDF
- [x] Test: trait object compatible (`Arc<dyn DocumentRenderer>`)
- [x] Test: default constructor

### Task 3: Update Module Exports (AC-3)
- [x] Update `mod.rs` để export `direct_pdf_renderer` module và `DirectPdfRenderer` struct

### Task 4: Integration Tests (AC-5)
- [x] Append tests to existing `renderer_integration_test.rs`
- [x] Test: output is valid PDF
- [x] Test: file size matches input
- [x] Test: output differs from PDFium output format
- [x] Test: multi-page PDF preserved

### Task 5: Documentation và AppContext (AC-6, AC-7)
- [x] Update TODO comment in `app_context.rs`
- [x] Update `README.md` trong renderer module

### Task 6: Final Verification
- [x] Run `cargo test` — all unit tests pass
- [x] Run `cargo test --test integration_tests` — integration tests pass
- [x] Run `cargo clippy` — no warnings
- [x] Run `cargo fmt` — code formatted
- [x] Verify file structure matches AC-1

## Dev Notes

### Architecture Alignment

**Strategy Pattern (AR-5, Decision 4):**
- `DocumentRenderer` trait = strategy interface
- `PdfiumRenderer` = control path (render with margins/color, ~2-3s)
- `DirectPdfRenderer` = fast path (pass-through raw PDF, ~0.5s)
- StrategySelector (Story 3.4) will choose between them based on printer capabilities

**Performance (NFR-1):**
- Direct PDF: ~0.5s per job (FR-3.2)
- Render: ~2-3s per job (FR-3.2)
- 4-6x performance improvement when Direct PDF is available

**Dependency Inversion:**
- `DirectPdfRenderer` implements `DocumentRenderer` trait
- Consumer code uses `Arc<dyn DocumentRenderer>` — doesn't know which strategy
- Testable via trait mocking

### Output Format — CRITICAL DISTINCTION

**PdfiumRenderer output format** (established in Story 3.2):
```
[page_count: u32 LE] [width: u32 LE] [height: u32 LE] [pixel data per page...]
```

**DirectPdfRenderer output format** (this story):
```
[raw PDF bytes — identical to input file]
```

These are **intentionally different**. The downstream consumer (PrinterEngine, Story 2.2/2.3 stubs) will need to handle both formats. This is by design — the printer engine will:
- For Direct PDF: send raw PDF bytes to printer (printer handles natively)
- For Render: send bitmap data to printer (rasterized)

**Do NOT try to make DirectPdfRenderer output bitmap format.** That defeats the entire purpose of the fast path.

### PDF Header Validation

Reuse the same validation pattern from `ReqwestDownloader`. **IMPORTANT:** Empty file check MUST come before `read_exact()` — otherwise `UnexpectedEof` maps to `NetworkError` instead of `ValidationError`.

```rust
fn validate_pdf_header(path: &Path) -> Result<(), InfrastructureError> {
    // 1. Check for empty file FIRST
    let metadata = std::fs::metadata(path)?;
    if metadata.len() == 0 {
        return Err(InfrastructureError::ValidationError(
            "File is empty".to_string(),
        ));
    }

    // 2. Read and check header
    let mut file = std::fs::File::open(path)?;
    let mut header = [0u8; 5];
    use std::io::Read;
    std::io::Read::read_exact(&mut file, &mut header).map_err(|_| {
        InfrastructureError::ValidationError(
            format!("File too small to contain PDF header: {:?}", path)
        )
    })?;
    if &header != b"%PDF-" {
        return Err(InfrastructureError::ValidationError(
            format!("File does not start with %PDF- header: {:?}", path)
        ));
    }
    Ok(())
}
```

**Note:** This is a simple header check, not full PDF validation. The goal is to catch obviously wrong files (HTML error pages, empty files), not to validate PDF structure.

### File Structure

```
src-tauri/src/infrastructure/renderer/
├── mod.rs                          # UPDATE: add direct_pdf_renderer module + re-export
├── document_renderer.rs            # Trait + types (NO CHANGE)
├── pdfium_renderer.rs              # Control path (NO CHANGE)
├── direct_pdf_renderer.rs          # NEW: Fast path implementation
│   └── #[cfg(test)] mod tests     # Inline unit tests
└── README.md                       # UPDATE: add DirectPdfRenderer docs

src-tauri/tests/integration/
└── renderer_integration_test.rs    # UPDATE: append Direct PDF integration tests

src-tauri/src/shared/
└── app_context.rs                  # UPDATE: TODO comment for Story 3.3
```

### Testing Strategy

**Unit Tests (Inline `#[cfg(test)]`):**
- Core behavior: raw bytes pass-through (byte-for-byte match)
- Error paths: invalid PDF, missing file, empty file
- Config independence: same output regardless of RenderConfig
- Performance: < 1s for ~100KB PDF
- Trait compatibility: `Arc<dyn DocumentRenderer>`

**Integration Tests (append to `renderer_integration_test.rs`):**
- Real file I/O with valid PDFs
- Compare Direct PDF vs PDFium output formats
- Multi-page PDF preservation

**Test PDF Creation:**
Reuse the same `create_test_pdf()` pattern from existing integration tests:
```rust
let pdf_content = b"%PDF-1.4\n\
    1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
    2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
    3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n\
    xref\n0 4\n\
    ...";
```

### Previous Story Learnings

**From Story 3.2 (MuPDF/PDFium Renderer):**
- **Library swap:** Originally planned MuPDF, switched to PDFium (`pdfium-render` 0.9) due to build compatibility on Windows. DirectPdfRenderer has NO external dependencies — pure `std::fs`.
- **Review fixes applied:** DPI=0 validation, NaN/Infinity checks, pixel skip on bitmap mismatch, test isolation with unique temp dirs. These fixes are in `document_renderer.rs` and `pdfium_renderer.rs` — DirectPdfRenderer doesn't need them (no rendering).
- **Output format:** PdfiumRenderer uses `[page_count: u32 LE][width][height][pixels]` binary format. DirectPdfRenderer returns raw PDF bytes. These are intentionally different.
- **Pattern to follow:** Trait impl in own file, inline tests, integration tests in separate file, module docs in README.

**From Story 3.1 (S3 Document Downloader):**
- **PDF header validation:** Same `%PDF-` check pattern — reuse approach.
- **Atomic download pattern:** Downloader creates `.tmp` → validates → renames to `.pdf`. DirectPdfRenderer reads the final `.pdf` file.
- **Error types:** Use `InfrastructureError::ValidationError` for invalid PDFs, `From<std::io::Error>` for file I/O.

### Key Constraints

**From FR-3.2 (Hybrid Rendering):**
- Direct PDF: ~0.5s per job
- Render: ~2-3s per job
- Auto-select strategy based on printer capabilities (Story 3.4)

**From AR-5 (Strategy Pattern):**
- `DocumentRenderer` trait with multiple implementations
- Both implementations must satisfy `Send + Sync` bounds
- Strategy selection deferred to Story 3.4

**From NFR-1 (Performance):**
- Direct PDF print: ~0.5s per job
- No MuPDF/PDFium dependency loaded for Direct PDF path
- Pure file read — minimal CPU usage

### Design Constraint: No Heavy Dependencies

**From epics:** *"No MuPDF dependency loaded (pure file read)"*

`DirectPdfRenderer` must NOT import or use `pdfium_render` (or any other PDF library). It is a pure `std::fs` + `std::io` implementation. This is a **design constraint**, not a runtime-testable requirement — Rust's module system ensures that code in `direct_pdf_renderer.rs` cannot accidentally pull in PDFium symbols.

**Verification approach:** The dev should confirm that `direct_pdf_renderer.rs` has NO `use pdfium_render::*` imports. The unit test `test_direct_pdf_trait_object_compatible` indirectly verifies this — if it compiles and runs without PDFium initialization, the dependency is absent.

### Cross-Module Dependency Warning

`validate_pdf_header` exists as a `pub fn` in `src-tauri/src/infrastructure/downloader/reqwest_downloader.rs`, but it is NOT re-exported from the downloader module. **Do NOT import it from the downloader module** — this would create an undesirable cross-module coupling (renderer → downloader internals). Instead, duplicate the 10-line validation function in `direct_pdf_renderer.rs`. This is intentional — the function is trivial and keeping modules decoupled is more important than avoiding duplication.

### What This Story Does NOT Do

- ❌ Does NOT implement strategy selection → Story 3.4
- ❌ Does NOT implement printer capability detection → Story 3.4
- ❌ Does NOT wire into AppContext → Use Case stories (3.8+)
- ❌ Does NOT implement actual printer data sending → PrinterEngine stories
- ❌ Does NOT implement RAII temp file management → Story 3.5
- ❌ Does NOT modify PdfiumRenderer or DocumentRenderer trait

### References

- [Source: `_bmad-output/planning-artifacts/epics.md` — Story 3.3 Acceptance Criteria]
- [Source: `_bmad-output/planning-artifacts/architecture.md` — Decision 4: Hybrid Rendering Strategy, AR-5]
- [Source: `_bmad-output/planning-artifacts/prds/prd-sapo-printer-2026-06-22/prd.md` — FR-3.2]
- [Source: `src-tauri/src/infrastructure/renderer/document_renderer.rs` — DocumentRenderer trait]
- [Source: `src-tauri/src/infrastructure/renderer/pdfium_renderer.rs` — Sibling implementation pattern]
- [Source: `src-tauri/src/infrastructure/downloader/reqwest_downloader.rs` — PDF header validation pattern]
- [Source: `src-tauri/src/shared/errors/infrastructure_error.rs` — Error types]
- [Source: `_bmad-output/implementation-artifacts/3-2-implement-mupdf-renderer-with-color-mode-support.md` — Previous story learnings]

## Dev Agent Record

### Agent Model Used

Qwen Code

### Debug Log References

No issues encountered. Implementation was straightforward — pure `std::fs` pass-through with PDF header validation.

### Completion Notes List

- ✅ Created `DirectPdfRenderer` unit struct implementing `DocumentRenderer` trait
- ✅ Render returns raw PDF bytes unchanged (pass-through)
- ✅ PDF header validation: empty file check before `read_exact()` to ensure correct error type
- ✅ 9 inline unit tests covering: raw bytes, header, invalid file, missing file, empty file, config ignored, performance, trait object, default constructor
- ✅ 4 integration tests: valid PDF output, file size match, output differs from PDFium, multi-page preservation
- ✅ Module exports updated in `mod.rs`
- ✅ AppContext TODO comment added for Story 3.3
- ✅ README.md updated with comparison table and usage example
- ✅ Fixed pre-existing clippy warnings in `app_context.rs`, `windows_credential_manager.rs`, `main.rs`
- ✅ All verification passes: `cargo test` (183 unit + 27 integration), `cargo clippy -D warnings`, `cargo fmt --check`

### File List

- `src-tauri/src/infrastructure/renderer/direct_pdf_renderer.rs` — NEW: DirectPdfRenderer implementation + 9 inline unit tests
- `src-tauri/src/infrastructure/renderer/mod.rs` — MODIFIED: added `direct_pdf_renderer` module + re-export
- `src-tauri/tests/integration/renderer_integration_test.rs` — MODIFIED: added `DirectPdfRenderer` import + 4 integration tests
- `src-tauri/src/shared/app_context.rs` — MODIFIED: added Story 3.3 TODO comment, fixed unused variable warning
- `src-tauri/src/infrastructure/renderer/README.md` — MODIFIED: added DirectPdfRenderer docs, comparison table, usage example
- `src-tauri/src/infrastructure/secrets/windows_credential_manager.rs` — MODIFIED: fixed clippy warnings (Default impl, removed unnecessary mut)
- `src-tauri/src/main.rs` — MODIFIED: fixed clippy warnings (unused field, unnecessary lazy eval)

### Change Log

- 2026-06-23: Implemented Story 3.3 — DirectPdfRenderer fast-path PDF strategy
- 2026-06-23: Fixed pre-existing clippy warnings to satisfy AC-8

### Review Findings

- [x] [Review][Decision] F-3: Directory path not explicitly rejected — `validate_pdf_header` does not check `metadata.is_file()`. On Windows, directories have `len() == 0` so they get "File is empty" error. On Unix, directories may have non-zero len and `File::open` on a directory may succeed. Should we add an explicit `is_file()` check with a clear error message? — dismissed, file paths come from downloader not user input
- [x] [Review][Patch] F-1: `read_exact` error cause discarded — `map_err(|_| ...)` at `direct_pdf_renderer.rs:66` unconditionally reports "File too small" regardless of actual error. Should match on `UnexpectedEof` vs other IO errors. [direct_pdf_renderer.rs:66] — fixed
- [x] [Review][Patch] F-2: Integration test indexes `pdfium_output` without bounds check — `pdfium_output[0..3]` at `renderer_integration_test.rs:481` without verifying `len() >= 4`. Add assertion before indexing. [renderer_integration_test.rs:481] — fixed
- [x] [Review][Defer] F-4: TOCTOU race between validate and read — spec explicitly defines two-step process (validate then read). Inherent to design, not actionable without spec change. — deferred, spec-defined pattern
- [x] [Review][Defer] F-5: IO errors misclassified as NetworkError — pre-existing `From<io::Error>` impl in `infrastructure_error.rs` maps all IO errors to `NetworkError`. Not caused by this story. — deferred, pre-existing infrastructure issue
- [x] [Review][Defer] F-6: No upper bound on file size — unbounded memory allocation via `std::fs::read`. Spec does not define maximum file size. — deferred, no spec-defined limit
- [x] [Review][Defer] F-7: Test temp dirs not cleaned up on panic — common Rust test pattern, not specific to this change. — deferred, project-wide pattern
- [x] [Review][Defer] F-8: Broken symlink → wrong error variant — same root cause as F-5 (pre-existing From impl). — deferred, pre-existing infrastructure issue

Status: done
