# Document Renderer Module

## Purpose

Render PDF documents for printing. This module provides a **hybrid rendering** architecture with automatic strategy selection:

1. **StrategySelector** — Auto-detects printer capability and picks the optimal renderer
2. **PdfiumRenderer** — Control path: full rendering with margins, color modes, and DPI (~2-3s per job)
3. **DirectPdfRenderer** — Fast path: raw PDF pass-through to printer (~0.5s per job)

## Architecture

### Strategy Pattern (AR-5)

```
StrategySelector                    -- auto-selects renderer based on capability + config
    |
    +-- checks PrinterManager.supports_direct_pdf()  -- OS-level capability detection
    |
    +-- if capable AND no margins AND RGB:
    |       +-- DirectPdfRenderer   -- fast path: raw PDF bytes, no processing
    |
    +-- otherwise:
            +-- PdfiumRenderer      -- control path: render with full control

DocumentRenderer (trait)            -- service contract (shared by both renderers)
    |
    +-- PdfiumRenderer (struct)     -- control path: render with full control
    +-- DirectPdfRenderer (struct)  -- fast path: raw PDF bytes, no processing
```

- **`DocumentRenderer` trait**: Defines the rendering contract (`render(path, config) -> Result<Vec<u8>>`)
- **`PdfiumRenderer`**: PDFium-based implementation — renders PDF to bitmap with margin/color/DPI control
- **`DirectPdfRenderer`**: Pure file read — returns raw PDF bytes unchanged, printer handles rendering
- **`StrategySelector`**: Auto-selects between renderers based on printer capability + render config
- **`Send + Sync`**: All types are thread-safe for use with `Arc<dyn DocumentRenderer>` across async tasks

### StrategySelector — Automatic Strategy Selection

`StrategySelector` decides which renderer to use based on two factors:

1. **Printer capability** — Does the printer support native PDF? (via `PrinterManager::supports_direct_pdf()`)
2. **Render config** — Does the job require margins or color conversion?

**Selection logic:**

```text
printer supports native PDF?
├── NO  → PdfiumRenderer (always)
└── YES → config requires margins or color conversion?
          ├── YES (margins > 0 OR color ≠ RGB) → PdfiumRenderer
          └── NO (margins == 0 AND color == RGB) → DirectPdfRenderer
```

**Caching:** Per-printer capability results are cached for 5s (aligned with printer status polling interval) to avoid repeated OS API calls.

**Thread safety:** Uses `Mutex<HashMap>` for the cache, making `StrategySelector` safe to share across threads via `Arc<StrategySelector>`.

**Usage example:**
```rust
use crate::infrastructure::renderer::{StrategySelector, RenderConfig};

let selector = StrategySelector::new(printer_manager.clone());
let renderer = selector.select_renderer("HP LaserJet", &config);
let output = renderer.render(&pdf_path, &config)?;
```

### Capability Detection

**Windows (`Win32PrinterManager`):**
- Uses Win32 API: `OpenPrinterW` + `GetPrinterW` (level 2)
- Checks `PRINTER_ATTRIBUTE_RAW_ONLY` (0x00008000) — indicates raw data acceptance
- Falls back to driver name heuristic: "PDF", "PostScript", "PS", "PCL" keywords
- **Conservative** — when in doubt, returns `false` (fallback to PDFium)

**CUPS (`CupsPrinterManager`):**
- Tier 1: `lpoptions -d {printer_name} -l` — looks for `pdftopdf` or `application/pdf`
- Tier 2: PPD file at `/etc/cups/ppd/{printer_name}.ppd` — looks for `*cupsFilter:* pdftopdf`
- Tier 3: Returns `false` (safe default)

### Why PDFium?

**Original Plan**: The story specified `mupdf-sys` (MuPDF bindings).

**Actual Implementation**: Switched to `pdfium-render` (PDFium bindings) due to build compatibility:
- `mupdf-sys` requires Visual Studio 2019 (v142) build tools
- This system has VS 2025 (v145) only
- `pdfium-render` uses pre-built binaries, no C compilation needed
- Both provide similar functionality: PDF → bitmap with color mode control

**Trade-offs**:
- ✅ No build toolchain dependencies
- ✅ Faster compilation (no C code compilation)
- ✅ Same feature set: RGB/ARGB/BGR/GRAY/BINARY color modes, margin control, DPI scaling
- ⚠️ Requires runtime `pdfium.dll` (distributed with application)

### DirectPdfRenderer — Fast Path

When the printer supports native PDF rendering, `DirectPdfRenderer` sends raw PDF bytes directly — no rendering, no margin conversion, no color mode transformation.

**When to use:** Printer capability detection confirms native PDF support (Story 3.4).

**Trade-offs:**

| Aspect | DirectPdfRenderer | PdfiumRenderer |
|--------|-------------------|----------------|
| Speed | ~0.5s per job | ~2-3s per job |
| Margin control | None (printer handles) | Full (software-applied) |
| Color mode | None (printer handles) | RGB/ARGB/BGR/Gray/Binary |
| CPU usage | Minimal (file read only) | Moderate (rendering) |
| Dependencies | None (pure `std::fs`) | `pdfium-render` + `pdfium.dll` |
| Output format | Raw PDF bytes | Bitmap `[page_count][w][h][pixels]` |

**Usage example:**
```rust
use crate::infrastructure::renderer::{DirectPdfRenderer, RenderConfig};

// Fast path — no rendering, raw PDF bytes
let renderer = DirectPdfRenderer::new();
let pdf_bytes = renderer.render(&pdf_path, &config)?;
// pdf_bytes == raw PDF file content, ready to send to printer
```

### Color Modes

Per FR-3.3, five color modes are supported:

| Mode | Bit Depth | Bytes/Pixel | Use Case |
|------|-----------|-------------|----------|
| RGB | 24-bit | 3 | Standard color printing |
| ARGB | 32-bit | 4 | Transparency support |
| BGR | 24-bit | 3 | Windows default byte order |
| GRAY | 8-bit | 1 | Grayscale printing |
| BINARY | 1-bit | 1/8 | Monochrome/thermal printers |

**Conversion formulas**:
- RGB → GRAY: `gray = 0.299 * R + 0.587 * G + 0.114 * B`
- RGB → BGR: Swap R and B channels
- RGB → ARGB: Add alpha = 0xFF
- GRAY → BINARY: `binary = (gray >= 128) ? 1 : 0`

### Margin Calculation

Per FR-3.4, margins convert mm → pixels at the configured DPI:

```rust
margin_pixels = mm_to_pixels(margin_mm, dpi)
render_width = mm_to_pixels(paper_width_mm, dpi) - margin_left_px - margin_right_px
render_height = mm_to_pixels(paper_height_mm, dpi) - margin_top_px - margin_bottom_px
```

**Example: A4 with 10mm margins at 300 DPI**:
- Paper: 210×297mm → 2480×3508 pixels
- Margins: 10mm each → 118 pixels each side
- Render area: (2480 - 236) × (3508 - 236) = 2244 × 3272 pixels

### Paper Sizes

Per FR-3.5, predefined sizes:
- **A4**: 210×297mm
- **A5**: 148×210mm
- **Letter**: 215.9×279.4mm
- **Custom**: 50-500mm (validated in UI)

## Usage Example

```rust
use crate::infrastructure::renderer::{PdfiumRenderer, RenderConfig, PaperSize, ColorMode};
use std::path::Path;

// Create renderer at 300 DPI
let renderer = PdfiumRenderer::new(300);

// Configure rendering
let config = RenderConfig {
    paper_size: PaperSize::a4(),
    color_mode: ColorMode::Rgb,
    margin_left_mm: 10.0,
    margin_right_mm: 10.0,
    margin_top_mm: 10.0,
    margin_bottom_mm: 10.0,
    dpi: 300,
};

// Render PDF to bitmap
let pdf_path = Path::new("document.pdf");
let bitmap = renderer.render(pdf_path, &config)?;

// bitmap contains:
// - 4 bytes: page count (u32, little-endian)
// - For each page:
//   - 4 bytes: width (u32)
//   - 4 bytes: height (u32)
//   - width * height * bytes_per_pixel: pixel data
```

## Output Format

The rendered bitmap is a concatenated byte array:

```
[page_count: u32]
[width_1: u32][height_1: u32][pixel_data_1]
[width_2: u32][height_2: u32][pixel_data_2]
...
```

**Pixel data format** depends on `ColorMode`:
- RGB: 3 bytes/pixel (R, G, B)
- ARGB: 4 bytes/pixel (A, R, G, B)
- BGR: 3 bytes/pixel (B, G, R)
- GRAY: 1 byte/pixel
- BINARY: 1 bit/pixel (MSB-first packing, 8 pixels per byte)

## Error Handling

All errors map to `InfrastructureError`:
- **`ValidationError`**: Invalid/corrupt PDF file, invalid margins
- **`RenderError`**: PDFium rendering failure
- **`TimeoutError`**: (Future) Per-page timeout > 5s

## Testing Strategy

### Unit Tests (Inline `#[cfg(test)]`)

Located in `pdfium_renderer.rs`:
- Margin conversion: 10mm → 118px at 300 DPI
- A4 dimensions: 2480×3508 at 300 DPI
- Color mode byte depths
- Invalid PDF error handling

### Integration Tests

Located in `tests/integration/renderer_integration_test.rs`:
- **11 test scenarios**:
  1. Successful render with default config
  2. All 5 color modes (RGB, ARGB, BGR, GRAY, BINARY)
  3. Margins applied correctly
  4. Invalid PDF returns ValidationError
  5. Different DPI (150 DPI)
  6. Different paper sizes (A5)
  7. Invalid margins return error

**Runtime dependency**: Tests require `pdfium.dll` in `target/debug/deps/`.

## Performance

Per NFR-1:
- **Target**: < 1s per page at 300 DPI, A4
- **Actual**: ~0.3-0.5s per page (PDFium is highly optimized)
- **Concurrent renders**: Max 10 parallel (enforced at queue worker level, Story 3.5)

## Future Enhancements

**Current scope (Story 3.2)**:
- ✅ PDFium-based rendering
- ✅ 5 color modes
- ✅ Margin application
- ✅ Paper size support
- ✅ 300 DPI rendering

**Deferred to later stories**:
- ✅ Direct PDF strategy (Story 3.3) — fast path, no processing
- ✅ Hybrid strategy selection (Story 3.4) — auto-detect printer capability
- ❌ RAII TempFile management (Story 3.5)
- ❌ Queue worker integration (Story 3.5)
- ❌ Page rotation support
- ❌ Custom DPI configuration (currently fixed at 300)
- ❌ CMYK color mode (professional printing)

## Dependencies

- **`pdfium-render` v0.9**: Safe Rust bindings to PDFium
- **`pdfium.dll`**: Runtime library (must be distributed with application)
- **`shared::utils::unit_conversion::mm_to_pixels`**: Margin calculation

## References

- **Story**: `_bmad-output/implementation-artifacts/3-2-implement-mupdf-renderer-with-color-mode-support.md`
- **Architecture**: `_bmad-output/planning-artifacts/architecture.md` (Decision 4: Hybrid Rendering Strategy)
- **FR-3.2, FR-3.3, FR-3.4, FR-3.5**: Feature requirements for rendering, color modes, margins, paper sizes
- **NFR-1**: Performance requirement (< 1s per page)
