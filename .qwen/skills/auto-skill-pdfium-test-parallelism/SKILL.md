---
name: pdfium-test-parallelism
description: Serialize PDFium library access in Rust tests to avoid global init panics from pdfium-render
source: auto-skill
extracted_at: '2026-06-23T10:36:39.489Z'
---

# PDFium Test Parallelism — Serialize Library Access

## When to use

When writing Rust tests (unit or integration) that call `Pdfium::default()` or any `pdfium-render` API. This applies to any test that renders PDFs via PDFium.

## Problem

`pdfium-render` v0.9 uses a global `OnceCell` internally for its FFI bindings. When Cargo runs tests in parallel (the default), multiple threads may call `Pdfium::default()` simultaneously. The second thread to arrive hits:

```
thread '...' panicked at pdfium-render-0.9.2\src\pdfium.rs:188:9:
assertion failed: BINDINGS.set(bindings).is_ok()
```

This causes test failures, access violations (`STATUS_ACCESS_VIOLATION` / `0xc0000005`), or process crashes.

## Solution

### Unit tests (inline `#[cfg(test)]`)

Add a test-local mutex using `OnceLock` to serialize all PDFium calls:

```rust
#[cfg(test)]
mod tests {
    use std::sync::{Mutex, OnceLock};

    static PDFIUM_TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

    fn pdfium_lock() -> std::sync::MutexGuard<'static, ()> {
        PDFIUM_TEST_MUTEX
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap()
    }

    #[test]
    fn test_that_uses_pdfium() {
        let _lock = pdfium_lock();
        // Now safe to call Pdfium::default() and render
        let pdfium = pdfium_render::prelude::Pdfium::default();
        // ...
    }

    #[test]
    fn test_that_does_not_use_pdfium() {
        // No lock needed — pure logic tests can run in parallel
    }
}
```

**Only lock tests that actually invoke PDFium.** Tests that exercise pure logic (e.g., margin calculations, config validation, selection logic without rendering) don't need the lock.

### Integration tests (`tests/integration/`)

Integration tests are a separate crate and can't share the unit test's mutex. Two options:

1. **Run serially**: `cargo test --test integration_tests -- --test-threads=1`
2. **Add a local mutex** in the integration test file (same pattern as above)

For this project, option 1 is preferred since integration tests are fast enough.

## Additional gotcha: Test PDFs must be valid

PDFium validates PDF structure strictly. A fake PDF (e.g., `%PDF-1.4\n` followed by zeros) will fail with:

```
PdfiumLibraryInternalError(FormatError)
```

**Use a valid minimal PDF** for all tests:

```rust
fn create_test_pdf(test_name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("sapo_tests_{}", test_name));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("test.pdf");

    let pdf_content = b"%PDF-1.4\n\
        1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
        2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
        3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n\
        xref\n0 4\n\
        0000000000 65535 f \n\
        0000000009 00000 n \n\
        0000000058 00000 n \n\
        0000000115 00000 n \n\
        trailer\n<< /Size 4 /Root 1 0 R >>\n\
        startxref\n190\n%%EOF";

    std::fs::write(&path, pdf_content).unwrap();
    path
}
```

## How to apply

When adding a new module that renders PDFs with PDFium:

1. Write the implementation first
2. For inline unit tests: add `PDFIUM_TEST_MUTEX` + `pdfium_lock()` helper
3. Acquire the lock at the start of any test that calls `Pdfium::default()` or `.render()`
4. For integration tests: run with `--test-threads=1` or add a local mutex
5. Always use valid minimal PDF content in test fixtures

## References

- Discovered during Story 3.4 implementation (`strategy_selector.rs` inline tests)
- `pdfium-render` source: `pdfium.rs:188` — `BINDINGS.set(bindings).is_ok()` assertion
- Related skill: `native-crate-build-compatibility` — covers the MSVC → pdfium-render pivot
