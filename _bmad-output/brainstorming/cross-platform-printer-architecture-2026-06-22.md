# Brainstorming Session: Cross-Platform Printer Architecture

**Date:** 2026-06-22  
**Topic:** Xây dựng Desktop App Cross-Platform cho In Hàng Loạt  
**Method:** SCAMPER + Mind Mapping + Pre-mortem  
**Duration:** ~60 phút  

---

## 📋 Executive Summary

### Bối cảnh dự án
SAPO Printer Client là desktop application cho phép in hàng loạt phiếu giao hàng, hiện đang được thiết kế theo DDD + Clean Architecture. Mục tiêu là xây dựng app **production-ready** với **feature parity 100%** trên Windows, macOS, và Linux.

### Key Decisions

| Decision Area | Choice | Rationale |
|--------------|--------|-----------|
| **Print Strategy** | Hybrid (Direct PDF + Render fallback) | Tối ưu performance, full control khi cần |
| **Rendering Engine** | MuPDF (bỏ PDFium) | Hiệu năng cao hơn 2-3x, multi-threading support |
| **Printer Discovery** | OS-native API | Windows (Win32 EnumPrinters) + macOS/Linux (CUPS) |
| **Architecture Pattern** | Clean Architecture + DDD | Domain độc lập, Infrastructure có thể thay thế |
| **Document Format** | PDF là chính | Đơn giản hóa rendering, 90% use case |
| **Auto-update** | Tauri Updater Plugin | Built-in cross-platform support |
| **Color Mode Strategy** | 5 modes với conversion pipeline | RGB, ARGB, BGR, GRAY, BINARY |

---

## 🎯 Requirements đã xác định

### 1. Mức độ ưu tiên cross-platform
✅ **Production-ready với feature parity 100%**
- Cả 3 nền tảng phải có feature đầy đủ
- Performance tương đương
- Support chính thức

### 2. Phạm vi printer support
✅ **Các máy in mà máy tính đang kết nối tới**
- Bao gồm USB local printer
- Bao gồm network printer đã add vào hệ thống
- Lấy từ OS printer list

### 3. Document types ưu tiên
✅ **PDF là chính**
- Đơn giản hóa rendering logic
- Cover 90% use case phiếu giao hàng

### 4. Timeline & Resources
✅ **Redesign hoàn toàn**
- Greenfield project
- Có thể thiết kế kiến trúc tối ưu từ đầu

---

## 🖥️ Printer Config UI (đã có)

### Màn hình cấu hình máy in

**Các trường thông tin:**

1. **Chọn máy in**
   - Dropdown danh sách máy in đang kết nối
   - Lấy từ OS (Windows/macOS/Linux)

2. **Color Mode**
   - RGB — Standard color (laser/inkjet)
   - ARGB — Transparency support (ít dùng)
   - BGR — Windows bitmap default
   - GRAY — Grayscale (thermal printer)
   - BINARY — Monochrome (label printer)

3. **Paper Settings**
   - Khổ giấy (Paper Size) — A4, A5, Letter, Custom
   - Chiều cao (Height) — mm
   - Chiều rộng (Width) — mm

4. **Margins**
   - Căn lề trái (Left Margin) — mm
   - Căn lề phải (Right Margin) — mm
   - Căn lề trên (Top Margin) — mm
   - Căn lề dưới (Bottom Margin) — mm

5. **Auto-update**
   - Tự động kiểm tra phiên bản mới
   - Tự động cập nhật app

---

## 🧠 SCAMPER Analysis

### S — Substitute (Thay thế)

**Thay PDFium bằng Hybrid Strategy:**

#### **Decision: Hybrid Print Strategy**

Thay vì chỉ dùng một phương pháp, sử dụng **Strategy Pattern** với 2 strategies:

**1. Direct PDF Printing** (Fast path - ưu tiên)
- Gửi PDF trực tiếp tới printer qua OS API
- ✅ **Nhanh nhất** — ~0.5s, zero rendering overhead
- ✅ **Chất lượng cao** — Printer tự render với resolution native
- ⚠️ **Điều kiện:** Printer hỗ trợ PostScript/PDF + Config là default (no custom margins/color)

**2. MuPDF Rendering** (Fallback - khi cần control)
- Render PDF với MuPDF → Apply config → Send raster
- ✅ **Full control** — Apply margins, color mode từ UI
- ✅ **Universal** — Work với mọi printer
- ⚠️ **Chậm hơn** — ~2-3s (rendering overhead)

#### **So sánh với PDFium:**

| Aspect | PDFium | MuPDF | Direct PDF | Winner |
|--------|--------|-------|------------|--------|
| Performance | Trung bình | Nhanh 2-3x | Nhanh nhất | ✅ Direct PDF |
| Memory | Memory hungry | Lightweight | Zero memory | ✅ Direct PDF |
| Multi-threading | Sequential | Native support | N/A | ✅ MuPDF |
| Control layout | ✅ | ✅ | ❌ | ⚠️ Depends |
| Cross-platform | ✅ | ✅ | ✅ | Tie |
| Rust binding | Mature | Chưa mature | Native OS | ⚠️ PDFium |
| Use case fit | Browser | Printing | Direct | ✅ Hybrid |

**Quyết định:** 
- **Bỏ PDFium** hoàn toàn
- **Ưu tiên Direct PDF** khi có thể (fast path)
- **Dùng MuPDF** khi cần control (fallback)

**Logic selector:**

```rust
if printer.supports_pdf() && config.is_default() {
    DirectPdfStrategy  // ~0.5s
} else {
    MuPdfRenderStrategy  // ~2-3s but full control
}
```

**Printer API Strategy:**

```
Windows: Win32 EnumPrinters API + StartDocPrinter (raw PDF)
macOS:   CUPS lpstat / CUPS API + lp -o raw
Linux:   CUPS lpstat / CUPS API + lp -o raw
```

---

### C — Combine (Kết hợp)

**Kết hợp nhiều strategy:**

1. **Direct PDF + MuPDF Render (Hybrid)**
   - Fast path khi có thể, fallback khi cần
   - Best of both worlds: speed + control

2. **Printer Detection + Capability Check**
   - Detect printer support PostScript/PDF
   - Auto-select strategy phù hợp

3. **Config Normalization + Platform Conversion**
   - Internal: mm (metric)
   - Platform-specific: convert khi cần (points, inches)

4. **Strategy Pattern + Metrics Logging**
   - Track performance per strategy
   - A/B test để optimize threshold

---

### A — Adapt (Điều chỉnh)

**Điều chỉnh từ Windows-only sang cross-platform:**

1. **Repository Pattern**
   ```
   WindowsPrinterRepository → PrinterRepository trait
   ├── WindowsPrinterRepositoryImpl (Win32)
   ├── CupsPrinterRepositoryImpl (macOS/Linux)
   ```

2. **Color Mode Value Object**
   - Thêm conversion logic: RGB ↔ BGR

3. **Units Normalization**
   - Internal: mm
   - Windows: 1/1000 inch
   - CUPS: points (1/72 inch)

### M — Modify (Thay đổi)

**Thay đổi flow:**

1. **Queue Worker**
   ```
   Download → Render (MuPDF) → Color Convert → Apply Margins → Print
   ```

2. **Status Reporting**
   - Thêm trạng thái: `RENDERING`, `CONVERTING_COLOR`

### P — Put to another use (Sử dụng khác)

**Component tái sử dụng:**

1. **MuPDF Renderer** → Dùng cho PDF preview trong app
2. **Printer Detection** → Export API cho web app check printer status
3. **Color Converter** → Dùng cho thumbnail generation

### E — Eliminate (Loại bỏ)

**Đơn giản hóa:**

1. ❌ **Bỏ PDFium Renderer** — Không phù hợp bulk printing, thay bằng Hybrid Strategy
2. ❌ **Bỏ ARGB mode** — Ít printer hỗ trợ alpha channel → chuyển sang RGB
3. ❌ **Bỏ ZPL/RAW renderer** — PDF only như đã thỏa thuận
4. ❌ **Bỏ "always render"** — Dùng Direct PDF khi có thể để tối ưu performance

### R — Reverse (Đảo ngược)

**Tiếp cận ngược:**

1. **Server-side rendering** → Server generate PNG, client chỉ in ảnh
   - ⚠️ Tăng bandwidth
   - ⚠️ Mất flexibility client-side config

2. **Let OS driver handle color** → Không convert, để driver tự xử lý
   - ⚠️ Inconsistent output cross-platform

**Quyết định:** Không reverse, giữ **Hybrid Strategy** — sử dụng client-side Direct PDF khi có thể (fast), render khi cần control (flexible).

---

## 🗺️ Mind Map: Cross-Platform Architecture

```
                    ┌─────────────────────────────────────┐
                    │  Cross-Platform Printer Client      │
                    │  (Tauri + Rust)                      │
                    │  Windows / macOS / Linux             │
                    └──────────────┬──────────────────────┘
                                   │
        ┌──────────────────────────┼──────────────────────────┐
        │                          │                          │
┌───────▼────────┐      ┌─────────▼─────────┐      ┌────────▼────────┐
│  1. PRINTER    │      │  2. PRINT         │      │  3. PRINTER     │
│     DISCOVERY  │      │     STRATEGY      │      │     CONFIG      │
└───────┬────────┘      └─────────┬─────────┘      └────────┬────────┘
        │                          │                         │
┌───────┼───────┐      ┌──────────┼──────────┐   ┌─────────┼─────────┐
│       │       │      │          │          │   │         │         │
▼       ▼       ▼      ▼          ▼          ▼   ▼         ▼         ▼
Win32   CUPS   CUPS   Direct    MuPDF     Auto  Paper   Margins   Color
Enum    macOS  Linux  PDF      Render   Select  Size    L/R/T/B   Mode
Printers              (Fast)   (Control) Strategy A4/A5           RGB/BGR
                      ~0.5s    ~2-3s                              GRAY/BIN
        │                          │                         │
┌───────▼────────┐      ┌─────────▼─────────┐      ┌────────▼────────┐
│  4. QUEUE      │      │  5. AUTO-UPDATE   │      │  6. TESTING     │
│     MANAGEMENT │      │     STRATEGY      │      │     STRATEGY    │
└───────┬────────┘      └─────────┬─────────┘      └────────┬────────┘
        │                          │                         │
┌───────┼───────┐      ┌──────────┼──────────┐   ┌─────────┼─────────┐
│       │       │      │          │          │   │         │         │
▼       ▼       ▼      ▼          ▼          ▼   ▼         ▼         ▼
Batch  Retry  Status  Tauri    Download   Install Mock    Real     CI/CD
Worker Logic  Real    Updater   Delta     Backup  Printer Printer  Matrix
Pool   Max3   time    Plugin   Package    Config  Tests   Tests   per OS
              WS
```

---

## 🏗️ Architecture Details

### 1. Printer Discovery (Cross-Platform)

**Domain Contract:**

```rust
// domain/printer/repository.rs
pub trait PrinterRepository {
    fn list_printers(&self) -> Result<Vec<Printer>>;
    fn get_printer(&self, name: &PrinterName) -> Result<Printer>;
    fn get_default_printer(&self) -> Result<Printer>;
}

pub struct Printer {
    pub id: PrinterId,
    pub name: PrinterName,
    pub status: PrinterStatus, // Online, Offline, Error
    pub capabilities: PrinterCapabilities,
}
```

**Infrastructure Implementations:**

```rust
// infrastructure/printer/windows.rs
#[cfg(target_os = "windows")]
pub struct WindowsPrinterRepository;

impl PrinterRepository for WindowsPrinterRepository {
    fn list_printers(&self) -> Result<Vec<Printer>> {
        // Call Win32 EnumPrinters API
        // PRINTER_INFO_2 structure
        unsafe {
            let mut needed = 0;
            let mut returned = 0;
            EnumPrintersW(
                PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS,
                null_mut(),
                2, // Level 2
                null_mut(),
                0,
                &mut needed,
                &mut returned,
            );
            // Parse and return Vec<Printer>
        }
    }
}

// infrastructure/printer/cups.rs
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub struct CupsPrinterRepository;

impl PrinterRepository for CupsPrinterRepository {
    fn list_printers(&self) -> Result<Vec<Printer>> {
        // Option 1: Call lpstat command
        let output = Command::new("lpstat")
            .args(&["-p", "-d"])
            .output()?;
        
        // Option 2: Use CUPS API via FFI
        // cups-sys crate (if available)
        
        // Parse output and return Vec<Printer>
    }
}
```

**Cross-platform gotchas:**

| Platform | API/Method | Notes |
|----------|-----------|-------|
| Windows | `EnumPrintersW` Win32 API | Registry-based, UTF-16 strings |
| macOS | `lpstat -p -d` or CUPS API | CUPS daemon must be running |
| Linux | `lpstat -p -d` or CUPS API | Different CUPS versions, permission issues |

---

### 2. Document Rendering (Hybrid Strategy)

**Decision:** Sử dụng **Hybrid Strategy** — Direct PDF khi có thể, fallback to Render khi cần.

**Rationale:**
- ✅ Direct PDF nhanh hơn (zero rendering overhead)
- ✅ Render PDF cho full control (margins, color mode)
- ✅ Best of both worlds

**Domain Contracts:**

```rust
// domain/document/print_strategy.rs
pub trait PrintStrategy {
    fn can_handle(&self, printer: &Printer, config: &PrinterConfig) -> bool;
    fn print(&self, pdf: &[u8], printer: &Printer, config: &PrinterConfig) -> Result<()>;
}

// domain/document/renderer.rs (for render strategy)
pub trait DocumentRenderer {
    fn render(
        &self,
        pdf: &[u8],
        config: &PrinterConfig,
    ) -> Result<RasterData>;
}

pub struct RasterData {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    pub format: ColorMode,
}
```

**Strategy Pattern Implementation:**

```rust
// infrastructure/printer/strategy_selector.rs
pub struct SmartPrintEngine {
    strategies: Vec<Box<dyn PrintStrategy>>,
}

impl SmartPrintEngine {
    pub fn new() -> Self {
        Self {
            strategies: vec![
                Box::new(DirectPdfStrategy::new()),
                Box::new(RenderPrintStrategy::new()),
            ],
        }
    }
    
    pub fn print(&self, pdf: &[u8], printer: &Printer, config: &PrinterConfig) -> Result<()> {
        for strategy in &self.strategies {
            if strategy.can_handle(printer, config) {
                return strategy.print(pdf, printer, config);
            }
        }
        Err(PrintError::NoSuitableStrategy)
    }
}
```

**Strategy 1: Direct PDF Printing (Fast Path)**

```rust
// infrastructure/printer/direct_pdf.rs
pub struct DirectPdfStrategy;

impl PrintStrategy for DirectPdfStrategy {
    fn can_handle(&self, printer: &Printer, config: &PrinterConfig) -> bool {
        // Only use direct print if:
        // 1. Printer supports PostScript/PDF
        // 2. Config is default (no custom margins/color)
        printer.capabilities.supports_pdf()
            && config.margins.is_default()
            && config.color_mode == ColorMode::default()
    }
    
    fn print(&self, pdf: &[u8], printer: &Printer, _config: &PrinterConfig) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            // Send raw PDF via Win32 API
            self.print_pdf_windows(pdf, printer)
        }
        
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            // Send via CUPS lp command with -o raw
            let temp_file = write_temp_pdf(pdf)?;
            Command::new("lp")
                .args(&[
                    "-d", &printer.name,
                    "-o", "raw",
                    temp_file.path().to_str().unwrap()
                ])
                .output()?;
            Ok(())
        }
    }
}

impl DirectPdfStrategy {
    #[cfg(target_os = "windows")]
    fn print_pdf_windows(&self, pdf: &[u8], printer: &Printer) -> Result<()> {
        // Use Win32 StartDocPrinter + WritePrinter
        // Send raw PDF data directly to printer queue
        // ... implementation
        Ok(())
    }
}
```

**Strategy 2: Render-then-Print (Universal Fallback)**

```rust
// infrastructure/printer/render_print.rs
pub struct RenderPrintStrategy {
    renderer: Arc<MuPdfRenderer>,
}

impl PrintStrategy for RenderPrintStrategy {
    fn can_handle(&self, _printer: &Printer, _config: &PrinterConfig) -> bool {
        true // Always works as fallback
    }
    
    fn print(&self, pdf: &[u8], printer: &Printer, config: &PrinterConfig) -> Result<()> {
        // Render with full config control
        let raster = self.renderer.render(pdf, config)?;
        
        // Send raster to printer
        self.print_raster(&printer.name, raster)
    }
}
```

**MuPDF Implementation (for Render Strategy):**

```rust
// infrastructure/renderer/mupdf.rs
pub struct MuPdfRenderer;

impl DocumentRenderer for MuPdfRenderer {
    fn render(&self, pdf: &[u8], config: &PrinterConfig) -> Result<RasterData> {
        // 1. Open PDF with MuPDF
        let doc = mupdf::Document::from_bytes(pdf)?;
        
        // 2. Calculate dimensions with margins
        let page_width = config.paper_settings.width_mm;
        let page_height = config.paper_settings.height_mm;
        let dpi = 300.0; // Standard print DPI
        
        // 3. Render to RGB bitmap
        let mut rgb_bitmap = doc.render_page(
            0, // page index
            dpi,
            page_width,
            page_height,
        )?;
        
        // 4. Apply margins
        let with_margins = self.apply_margins(rgb_bitmap, &config.margins)?;
        
        // 5. Convert color mode
        let final_data = self.convert_color(with_margins, config.color_mode)?;
        
        Ok(RasterData {
            width: final_data.width,
            height: final_data.height,
            data: final_data.pixels,
            format: config.color_mode,
        })
    }
}

impl MuPdfRenderer {
    fn apply_margins(&self, bitmap: Bitmap, margins: &Margins) -> Result<Bitmap> {
        // Add white space for margins
        // Convert mm to pixels at current DPI
        let dpi = 300.0;
        let left_px = (margins.left_mm / 25.4 * dpi) as u32;
        let right_px = (margins.right_mm / 25.4 * dpi) as u32;
        let top_px = (margins.top_mm / 25.4 * dpi) as u32;
        let bottom_px = (margins.bottom_mm / 25.4 * dpi) as u32;
        
        // Create new bitmap with margins
        let new_width = bitmap.width + left_px + right_px;
        let new_height = bitmap.height + top_px + bottom_px;
        
        // Copy original to center, fill margins with white
        // ... implementation
    }
    
    fn convert_color(&self, bitmap: Bitmap, mode: ColorMode) -> Result<Bitmap> {
        match mode {
            ColorMode::RGB => Ok(bitmap), // Already RGB
            ColorMode::BGR => self.rgb_to_bgr(bitmap),
            ColorMode::GRAY => self.rgb_to_grayscale(bitmap),
            ColorMode::BINARY => self.rgb_to_binary(bitmap),
            ColorMode::ARGB => self.rgb_to_argb(bitmap),
        }
    }
    
    fn rgb_to_bgr(&self, bitmap: Bitmap) -> Result<Bitmap> {
        // Swap R and B channels
        let mut data = bitmap.data;
        for chunk in data.chunks_exact_mut(3) {
            chunk.swap(0, 2); // Swap R and B
        }
        Ok(Bitmap { data, ..bitmap })
    }
    
    fn rgb_to_grayscale(&self, bitmap: Bitmap) -> Result<Bitmap> {
        // Luminosity method: 0.299R + 0.587G + 0.114B
        let gray_data: Vec<u8> = bitmap.data
            .chunks_exact(3)
            .map(|rgb| {
                (0.299 * rgb[0] as f32 
                + 0.587 * rgb[1] as f32 
                + 0.114 * rgb[2] as f32) as u8
            })
            .collect();
        Ok(Bitmap { data: gray_data, ..bitmap })
    }
    
    fn rgb_to_binary(&self, bitmap: Bitmap) -> Result<Bitmap> {
        // Apply threshold (e.g., 128)
        let gray = self.rgb_to_grayscale(bitmap)?;
        let binary_data: Vec<u8> = gray.data
            .iter()
            .map(|&px| if px > 128 { 255 } else { 0 })
            .collect();
        Ok(Bitmap { data: binary_data, ..gray })
    }
}
```

**Performance comparison:**

| Strategy | Speed | Quality | Use Case |
|----------|-------|---------|----------|
| **Direct PDF** | ~0.5s | High (printer native) | Default margins, modern printer |
| **Render Print** | ~2-3s | Controlled | Custom margins/color, old printer |

**Performance optimization for Render Strategy:**

```rust
// Parallel batch rendering
use rayon::prelude::*;

pub async fn render_batch(
    pdfs: Vec<Vec<u8>>,
    config: &PrinterConfig,
) -> Result<Vec<RasterData>> {
    pdfs.par_iter()
        .map(|pdf| renderer.render(pdf, config))
        .collect()
}
```

**Decision tree:**

```
PDF + Config
     |
     ├─> Printer supports PDF?
     |   ├─> NO → Render Strategy
     |   └─> YES
     |        |
     |        ├─> Custom margins?
     |        |   ├─> YES → Render Strategy
     |        |   └─> NO
     |        |        |
     |        |        ├─> Custom color mode?
     |        |        |   ├─> YES → Render Strategy
     |        |        |   └─> NO → Direct PDF Strategy ✅ (fastest)
```

**Logging & Metrics:**

```rust
impl SmartPrintEngine {
    pub fn print(&self, pdf: &[u8], printer: &Printer, config: &PrinterConfig) -> Result<()> {
        let start = Instant::now();
        
        for strategy in &self.strategies {
            if strategy.can_handle(printer, config) {
                let strategy_name = strategy.name();
                tracing::info!("Using print strategy: {}", strategy_name);
                
                let result = strategy.print(pdf, printer, config);
                
                let duration = start.elapsed();
                tracing::info!(
                    "Print completed in {:?} using {}",
                    duration,
                    strategy_name
                );
                
                // Metrics
                metrics::histogram!("print_duration_seconds", duration.as_secs_f64())
                    .with_label("strategy", strategy_name);
                
                return result;
            }
        }
        
        Err(PrintError::NoSuitableStrategy)
    }
}
```

---



**MuPDF Implementation:**

```rust
// infrastructure/renderer/mupdf.rs
pub struct MuPdfRenderer;

impl DocumentRenderer for MuPdfRenderer {
    fn render(&self, pdf: &[u8], config: &PrinterConfig) -> Result<RasterData> {
        // 1. Open PDF with MuPDF
        let doc = mupdf::Document::from_bytes(pdf)?;
        
        // 2. Calculate dimensions with margins
        let page_width = config.paper_settings.width_mm;
        let page_height = config.paper_settings.height_mm;
        let dpi = 300.0; // Standard print DPI
        
        // 3. Render to RGB bitmap
        let mut rgb_bitmap = doc.render_page(
            0, // page index
            dpi,
            page_width,
            page_height,
        )?;
        
        // 4. Apply margins
        let with_margins = self.apply_margins(rgb_bitmap, &config.margins)?;
        
        // 5. Convert color mode
        let final_data = self.convert_color(with_margins, config.color_mode)?;
        
        Ok(RasterData {
            width: final_data.width,
            height: final_data.height,
            data: final_data.pixels,
            format: config.color_mode,
        })
    }
}

impl MuPdfRenderer {
    fn apply_margins(&self, bitmap: Bitmap, margins: &Margins) -> Result<Bitmap> {
        // Add white space for margins
        // Convert mm to pixels at current DPI
        let dpi = 300.0;
        let left_px = (margins.left_mm / 25.4 * dpi) as u32;
        let right_px = (margins.right_mm / 25.4 * dpi) as u32;
        let top_px = (margins.top_mm / 25.4 * dpi) as u32;
        let bottom_px = (margins.bottom_mm / 25.4 * dpi) as u32;
        
        // Create new bitmap with margins
        let new_width = bitmap.width + left_px + right_px;
        let new_height = bitmap.height + top_px + bottom_px;
        
        // Copy original to center, fill margins with white
        // ... implementation
    }
    
    fn convert_color(&self, bitmap: Bitmap, mode: ColorMode) -> Result<Bitmap> {
        match mode {
            ColorMode::RGB => Ok(bitmap), // Already RGB
            ColorMode::BGR => self.rgb_to_bgr(bitmap),
            ColorMode::GRAY => self.rgb_to_grayscale(bitmap),
            ColorMode::BINARY => self.rgb_to_binary(bitmap),
            ColorMode::ARGB => self.rgb_to_argb(bitmap),
        }
    }
    
    fn rgb_to_bgr(&self, bitmap: Bitmap) -> Result<Bitmap> {
        // Swap R and B channels
        let mut data = bitmap.data;
        for chunk in data.chunks_exact_mut(3) {
            chunk.swap(0, 2); // Swap R and B
        }
        Ok(Bitmap { data, ..bitmap })
    }
    
    fn rgb_to_grayscale(&self, bitmap: Bitmap) -> Result<Bitmap> {
        // Luminosity method: 0.299R + 0.587G + 0.114B
        let gray_data: Vec<u8> = bitmap.data
            .chunks_exact(3)
            .map(|rgb| {
                (0.299 * rgb[0] as f32 
                + 0.587 * rgb[1] as f32 
                + 0.114 * rgb[2] as f32) as u8
            })
            .collect();
        Ok(Bitmap { data: gray_data, ..bitmap })
    }
    
    fn rgb_to_binary(&self, bitmap: Bitmap) -> Result<Bitmap> {
        // Apply threshold (e.g., 128)
        let gray = self.rgb_to_grayscale(bitmap)?;
        let binary_data: Vec<u8> = gray.data
            .iter()
            .map(|&px| if px > 128 { 255 } else { 0 })
            .collect();
        Ok(Bitmap { data: binary_data, ..gray })
    }
}
```

**Performance optimization:**

```rust
// Parallel batch rendering
use rayon::prelude::*;

pub async fn render_batch(
    pdfs: Vec<Vec<u8>>,
    config: &PrinterConfig,
) -> Result<Vec<RasterData>> {
    pdfs.par_iter()
        .map(|pdf| renderer.render(pdf, config))
        .collect()
}
```

---

### 3. Printer Config Management

**Domain Model:**

```rust
// domain/printer/config.rs
pub struct PrinterConfig {
    pub printer_name: PrinterName,
    pub color_mode: ColorMode,
    pub paper_settings: PaperSettings,
    pub margins: Margins,
}

pub struct PaperSettings {
    pub size: PaperSize,
    pub width_mm: f32,  // Always mm internally
    pub height_mm: f32,
}

pub enum PaperSize {
    A4,      // 210 x 297 mm
    A5,      // 148 x 210 mm
    Letter,  // 215.9 x 279.4 mm
    Custom { width_mm: f32, height_mm: f32 },
}

pub struct Margins {
    pub left_mm: f32,
    pub right_mm: f32,
    pub top_mm: f32,
    pub bottom_mm: f32,
}

pub enum ColorMode {
    RGB,     // Red-Green-Blue (24-bit)
    ARGB,    // Alpha-Red-Green-Blue (32-bit)
    BGR,     // Blue-Green-Red (24-bit, Windows default)
    GRAY,    // Grayscale (8-bit)
    BINARY,  // Monochrome (1-bit)
}
```

**Cross-platform units conversion:**

```rust
impl PrinterConfig {
    // Convert internal mm to platform-specific units
    pub fn to_platform_margins(&self) -> PlatformMargins {
        #[cfg(target_os = "windows")]
        {
            // Windows uses 1/1000 inch
            PlatformMargins {
                left: (self.margins.left_mm / 25.4 * 1000.0) as i32,
                right: (self.margins.right_mm / 25.4 * 1000.0) as i32,
                top: (self.margins.top_mm / 25.4 * 1000.0) as i32,
                bottom: (self.margins.bottom_mm / 25.4 * 1000.0) as i32,
            }
        }
        
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            // CUPS uses points (1/72 inch)
            PlatformMargins {
                left: (self.margins.left_mm / 25.4 * 72.0) as i32,
                right: (self.margins.right_mm / 25.4 * 72.0) as i32,
                top: (self.margins.top_mm / 25.4 * 72.0) as i32,
                bottom: (self.margins.bottom_mm / 25.4 * 72.0) as i32,
            }
        }
    }
}
```

---

### 4. Queue Management

**Architecture:**

```rust
// application/queue/worker.rs
pub struct QueueWorker {
    renderer: Arc<dyn DocumentRenderer>,
    printer_engine: Arc<dyn PrinterEngine>,
    downloader: Arc<dyn DocumentDownloader>,
    config_repo: Arc<dyn ConfigRepository>,
}

impl QueueWorker {
    pub async fn process_job(&self, job: PrintJob) -> Result<()> {
        // 1. Download PDF
        self.emit_status(PrintJobStarted { job_id: job.id })?;
        let pdf_data = self.downloader
            .download(&job.document_url)
            .await?;
        
        // 2. Load printer config
        self.emit_status(PrintJobDownloaded { job_id: job.id })?;
        let config = self.config_repo
            .get_by_printer(&job.printer_name)?;
        
        // 3. Render with config (includes color + margins)
        self.emit_status(PrintJobRendering { job_id: job.id })?;
        let raster = self.renderer
            .render(&pdf_data, &config)?;
        
        // 4. Print
        self.emit_status(PrintJobSubmitted { job_id: job.id })?;
        self.printer_engine
            .print(&job.printer_name, raster)?;
        
        // 5. Update status
        self.emit_status(PrintJobCompleted { job_id: job.id })?;
        
        Ok(())
    }
    
    fn emit_status(&self, event: impl DomainEvent) -> Result<()> {
        self.event_bus.publish(event)
    }
}

// Batch processing with parallel rendering
pub struct BatchProcessor {
    worker_pool: ThreadPool,
}

impl BatchProcessor {
    pub async fn process_batch(&self, jobs: Vec<PrintJob>) -> Result<()> {
        // Render all PDFs in parallel (CPU-bound)
        let renders: Vec<_> = jobs
            .par_iter()
            .map(|job| self.render_job(job))
            .collect::<Result<_>>()?;
        
        // Print sequentially (I/O-bound, printer bottleneck)
        for (job, raster) in jobs.iter().zip(renders.iter()) {
            self.printer_engine.print(&job.printer_name, raster)?;
        }
        
        Ok(())
    }
}
```

**Retry logic:**

```rust
// domain/print_job/aggregate.rs
impl PrintJob {
    const MAX_RETRY: u8 = 3;
    
    pub fn can_retry(&self) -> bool {
        self.retry_count < Self::MAX_RETRY
            && matches!(self.status, PrintStatus::Failed)
    }
    
    pub fn retry(&mut self) -> Result<(), DomainError> {
        if !self.can_retry() {
            return Err(DomainError::MaxRetryExceeded);
        }
        
        self.retry_count += 1;
        self.status = PrintStatus::Pending;
        self.add_event(PrintJobRetried { 
            job_id: self.id, 
            retry_count: self.retry_count 
        });
        
        Ok(())
    }
}
```

---

### 5. Auto-Update Strategy

**Tauri Configuration:**

```json
// tauri.conf.json
{
  "tauri": {
    "updater": {
      "active": true,
      "endpoints": [
        "https://releases.sapo.vn/printer-client/{{target}}/{{current_version}}"
      ],
      "dialog": true,
      "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEFCQ0RFRkdISUpLTE0KUldTZ...",
      "windows": {
        "installMode": "passive"
      }
    }
  }
}
```

**Application Layer Use Case:**

```rust
// application/update/check_update.rs
pub struct CheckForUpdateUseCase {
    update_service: Arc<dyn UpdateService>,
}

impl CheckForUpdateUseCase {
    pub async fn execute(&self) -> Result<UpdateInfo> {
        let latest_version = self.update_service
            .check_latest()
            .await?;
        
        let current_version = env!("CARGO_PKG_VERSION");
        
        if latest_version > current_version {
            Ok(UpdateInfo::Available { 
                version: latest_version,
                release_notes: self.update_service
                    .get_release_notes(&latest_version)
                    .await?
            })
        } else {
            Ok(UpdateInfo::UpToDate)
        }
    }
}

pub struct DownloadAndInstallUpdateUseCase {
    update_service: Arc<dyn UpdateService>,
    config_backup: Arc<dyn ConfigBackupService>,
}

impl DownloadAndInstallUpdateUseCase {
    pub async fn execute(&self) -> Result<()> {
        // 1. Backup config before update
        self.config_backup.backup_all().await?;
        
        // 2. Download update
        let update_bundle = self.update_service
            .download_latest()
            .await?;
        
        // 3. Verify signature
        self.update_service
            .verify_signature(&update_bundle)?;
        
        // 4. Install (will restart app)
        self.update_service
            .install(update_bundle)
            .await?;
        
        Ok(())
    }
}
```

**Cross-platform implementations:**

```rust
// infrastructure/update/tauri_updater.rs
pub struct TauriUpdateService;

impl UpdateService for TauriUpdateService {
    async fn check_latest(&self) -> Result<Version> {
        #[cfg(target_os = "windows")]
        {
            // Check NSIS installer endpoint
            let url = format!(
                "https://releases.sapo.vn/printer-client/windows-x86_64/{}/latest.json",
                env!("CARGO_PKG_VERSION")
            );
            // ... implementation
        }
        
        #[cfg(target_os = "macos")]
        {
            // Check DMG/App bundle endpoint
            let url = format!(
                "https://releases.sapo.vn/printer-client/darwin-x86_64/{}/latest.json",
                env!("CARGO_PKG_VERSION")
            );
            // ... implementation
        }
        
        #[cfg(target_os = "linux")]
        {
            // Check AppImage endpoint
            let url = format!(
                "https://releases.sapo.vn/printer-client/linux-x86_64/{}/latest.json",
                env!("CARGO_PKG_VERSION")
            );
            // ... implementation
        }
    }
}
```

---

### 6. Testing Strategy

**Test pyramid:**

```
        ┌────────────┐
        │    E2E     │  → Real printer integration tests
        └────────────┘
      ┌──────────────┐
      │ Integration  │  → Mock printer, real rendering
      └──────────────┘
    ┌──────────────────┐
    │      Unit        │  → Domain logic, value objects
    └──────────────────┘
```

**Unit tests:**

```rust
// domain/print_job/aggregate_test.rs
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cannot_retry_after_max_attempts() {
        let mut job = PrintJob::new(/* ... */);
        job.retry_count = 3;
        job.status = PrintStatus::Failed;
        
        assert!(job.retry().is_err());
    }
    
    #[test]
    fn test_cannot_print_completed_job() {
        let job = PrintJob::completed(/* ... */);
        
        assert!(job.can_print() == false);
    }
}
```

**Integration tests with mock printer:**

```rust
// tests/integration/printer_test.rs
#[tokio::test]
async fn test_print_workflow_with_mock_printer() {
    // Arrange
    let mock_printer = MockPrinterEngine::new();
    let renderer = MuPdfRenderer::new();
    let worker = QueueWorker::new(renderer, mock_printer);
    
    let job = PrintJob::new(
        "test-printer",
        "https://example.com/test.pdf",
    );
    
    // Act
    let result = worker.process_job(job).await;
    
    // Assert
    assert!(result.is_ok());
    assert_eq!(mock_printer.print_count(), 1);
}
```

**Cross-platform CI/CD:**

```yaml
# .github/workflows/ci.yml
name: Cross-Platform CI

on: [push, pull_request]

jobs:
  test:
    strategy:
      matrix:
        os: [windows-latest, macos-latest, ubuntu-latest]
        
    runs-on: ${{ matrix.os }}
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      
      - name: Install dependencies (Linux)
        if: matrix.os == 'ubuntu-latest'
        run: |
          sudo apt-get update
          sudo apt-get install -y libcups2-dev
      
      - name: Run tests
        run: cargo test --all-features
      
      - name: Test printer discovery
        run: cargo test test_list_printers -- --ignored
      
      - name: Test MuPDF rendering
        run: cargo test test_render_pdf
      
      - name: Test color conversion
        run: cargo test test_color_modes

  build:
    needs: test
    strategy:
      matrix:
        include:
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            artifact: sapo-printer-setup.exe
          - os: macos-latest
            target: x86_64-apple-darwin
            artifact: sapo-printer.dmg
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            artifact: sapo-printer.AppImage
    
    runs-on: ${{ matrix.os }}
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Build Tauri app
        run: npm run tauri build -- --target ${{ matrix.target }}
      
      - name: Upload artifact
        uses: actions/upload-artifact@v3
        with:
          name: ${{ matrix.artifact }}
          path: src-tauri/target/release/bundle/
```

---

## ⚠️ Pre-mortem: Failure Scenarios

### Scenario 1: Printer không detect được trên Linux

**Tình huống:**
User cài app trên Ubuntu, nhưng không thấy máy in trong dropdown.

**Nguyên nhân khả dĩ:**
- CUPS daemon không chạy
- Permission denied khi access `/etc/cups/`
- Printer chưa được add vào CUPS system

**Giải pháp phòng ngừa:**
```rust
// Check CUPS status before listing printers
pub fn check_cups_status() -> Result<CupsStatus> {
    let output = Command::new("systemctl")
        .args(&["is-active", "cups"])
        .output()?;
    
    if !output.status.success() {
        return Ok(CupsStatus::NotRunning);
    }
    
    Ok(CupsStatus::Running)
}

// Show helpful error message
if cups_status == CupsStatus::NotRunning {
    show_error_dialog(
        "CUPS service không chạy",
        "Vui lòng chạy lệnh: sudo systemctl start cups"
    );
}
```

**Mitigation:**
- Thêm healthcheck UI: "System Status" panel
- Documentation: Linux setup guide
- Fallback: Manual printer name input

---

### Scenario 2: MuPDF crash với PDF corrupt

**Tình huống:**
Server generate PDF bị lỗi → MuPDF crash → app crash.

**Nguyên nhân khả dĩ:**
- PDF không đúng chuẩn (malformed)
- PDF version mới hơn MuPDF support
- PDF có embedded font lỗi

**Giải pháp phòng ngừa:**
```rust
// Validate PDF before rendering
pub fn validate_pdf(data: &[u8]) -> Result<PdfInfo> {
    // Check magic bytes
    if !data.starts_with(b"%PDF-") {
        return Err(RenderError::InvalidPdf);
    }
    
    // Try to open without rendering
    let doc = mupdf::Document::from_bytes(data)
        .map_err(|e| RenderError::MuPdfError(e))?;
    
    Ok(PdfInfo {
        page_count: doc.page_count(),
        version: doc.version(),
    })
}

// Graceful error handling
impl QueueWorker {
    async fn process_job(&self, job: PrintJob) -> Result<()> {
        let pdf_data = self.downloader.download(&job.document_url).await?;
        
        // Validate first
        if let Err(e) = validate_pdf(&pdf_data) {
            self.emit_event(PrintJobFailed { 
                job_id: job.id, 
                reason: format!("Invalid PDF: {}", e) 
            })?;
            return Ok(()); // Don't crash, just mark as failed
        }
        
        // Render with timeout
        let raster = tokio::time::timeout(
            Duration::from_secs(30),
            self.renderer.render(&pdf_data, &config)
        ).await??;
        
        // ...
    }
}
```

**Mitigation:**
- Server-side PDF validation trước khi send
- Client-side validation + graceful degradation
- Timeout cho rendering operations
- Detailed error logging cho debug

---

### Scenario 3: Color sai trên Windows (đỏ thành xanh)

**Tình huống:**
Print trên Windows, màu đỏ hiện thành màu xanh dương.

**Nguyên nhân:**
- Không convert RGB → BGR
- Windows GDI expect BGR byte order
- Config sai color mode

**Giải pháp phòng ngừa:**
```rust
// Auto-detect platform color mode
impl PrinterConfig {
    pub fn default_color_mode() -> ColorMode {
        #[cfg(target_os = "windows")]
        return ColorMode::BGR;
        
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        return ColorMode::RGB;
    }
}

// Unit test per platform
#[cfg(test)]
mod tests {
    #[test]
    fn test_color_conversion_rgb_to_bgr() {
        let rgb = vec![255, 0, 0]; // Pure red
        let bgr = convert_rgb_to_bgr(&rgb);
        assert_eq!(bgr, vec![0, 0, 255]); // Blue in BGR
    }
    
    #[cfg(target_os = "windows")]
    #[test]
    fn test_windows_default_is_bgr() {
        assert_eq!(
            PrinterConfig::default_color_mode(),
            ColorMode::BGR
        );
    }
}
```

**Mitigation:**
- Platform-specific unit tests
- Visual regression tests (compare screenshots)
- User can override color mode manually
- Documentation: "Nếu màu sai, thử đổi Color Mode"

---

### Scenario 4: Margins không đúng trên macOS

**Tình huống:**
Set margin 10mm, nhưng in ra margin > 10mm.

**Nguyên nhân:**
- Units conversion sai: mm → points
- 1 point = 1/72 inch, không phải 1mm
- Sai công thức: `10mm ≠ 10 points`

**Giải pháp phòng ngừa:**
```rust
// Correct conversion with tests
impl Margins {
    pub fn to_points(&self) -> MarginsInPoints {
        // 1 inch = 25.4 mm
        // 1 inch = 72 points
        // => 1 mm = 72/25.4 points ≈ 2.834645669 points
        const MM_TO_POINTS: f32 = 72.0 / 25.4;
        
        MarginsInPoints {
            left: self.left_mm * MM_TO_POINTS,
            right: self.right_mm * MM_TO_POINTS,
            top: self.top_mm * MM_TO_POINTS,
            bottom: self.bottom_mm * MM_TO_POINTS,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_mm_to_points_conversion() {
        let margins = Margins {
            left_mm: 10.0,
            right_mm: 10.0,
            top_mm: 10.0,
            bottom_mm: 10.0,
        };
        
        let points = margins.to_points();
        
        // 10mm ≈ 28.35 points
        assert!((points.left - 28.35).abs() < 0.1);
    }
}
```

**Mitigation:**
- Integration test với real printer
- Measure physical output
- Conversion constants có unit tests
- Documentation rõ ràng về units

---

### Scenario 5: Performance chậm với batch 50 PDF

**Tình huống:**
Render 50 PDF mất > 2 phút, user complain.

**Nguyên nhân:**
- MuPDF render sequential (không parallel)
- PDF size lớn (>5MB each)
- Không cache rendered data

**Giải pháp phòng ngừa:**
```rust
// Parallel rendering với rayon
use rayon::prelude::*;

pub fn render_batch_parallel(
    pdfs: Vec<Vec<u8>>,
    config: &PrinterConfig,
) -> Result<Vec<RasterData>> {
    pdfs.par_iter()
        .map(|pdf| {
            let renderer = MuPdfRenderer::new();
            renderer.render(pdf, config)
        })
        .collect()
}

// Progress reporting
impl QueueWorker {
    async fn process_batch(&self, jobs: Vec<PrintJob>) -> Result<()> {
        let total = jobs.len();
        
        for (idx, job) in jobs.iter().enumerate() {
            self.emit_event(BatchProgress {
                current: idx + 1,
                total,
                percentage: ((idx + 1) as f32 / total as f32 * 100.0) as u8,
            })?;
            
            self.process_job(job).await?;
        }
        
        Ok(())
    }
}

// Benchmark test
#[cfg(test)]
mod bench {
    use criterion::{black_box, criterion_group, criterion_main, Criterion};
    
    fn bench_render_batch(c: &mut Criterion) {
        c.bench_function("render 50 PDFs", |b| {
            b.iter(|| {
                render_batch_parallel(
                    black_box(load_test_pdfs(50)),
                    black_box(&default_config())
                )
            });
        });
    }
    
    criterion_group!(benches, bench_render_batch);
    criterion_main!(benches);
}
```

**Mitigation:**
- Parallel rendering (CPU-bound)
- Stream processing large PDFs
- Progress bar real-time
- Performance test trong CI (benchmark thresholds)

---

### Scenario 6: Update fail trên Linux AppImage

**Tình huống:**
Auto-update download xong nhưng không install được.

**Nguyên nhân:**
- AppImage không có write permission
- AppImage mounted as read-only
- AppImageUpdate không installed

**Giải pháp phòng ngừa:**
```rust
// Check update capability before attempting
pub fn can_auto_update() -> bool {
    #[cfg(target_os = "linux")]
    {
        // Check if running as AppImage
        let is_appimage = std::env::var("APPIMAGE").is_ok();
        
        if is_appimage {
            // Check if AppImageUpdate available
            Command::new("appimageupdatetool")
                .arg("--version")
                .output()
                .is_ok()
        } else {
            false // Not AppImage, can't auto-update
        }
    }
    
    #[cfg(not(target_os = "linux"))]
    {
        true // Windows/macOS have native update
    }
}

// Show manual update instruction
if !can_auto_update() {
    show_dialog(
        "Update khả dụng",
        "Vui lòng download version mới tại: https://releases.sapo.vn/printer-client/latest"
    );
}
```

**Mitigation:**
- Detect AppImage environment
- Fallback to manual update link
- Documentation: Linux update guide
- Consider alternative: snap/flatpak với auto-update built-in

---

## 📊 Risk Matrix

| Scenario | Likelihood | Impact | Priority | Mitigation Status |
|----------|-----------|--------|----------|------------------|
| Printer không detect (Linux) | High | High | 🔴 P0 | ✅ Healthcheck UI + docs |
| MuPDF crash | Medium | High | 🟡 P1 | ✅ Validation + timeout |
| Color sai (Windows) | Medium | Medium | 🟡 P1 | ✅ Platform tests |
| Margins không đúng | Low | Medium | 🟢 P2 | ✅ Unit conversion tests |
| Performance chậm | Medium | High | 🟡 P1 | ✅ Parallel rendering |
| Update fail (Linux) | High | Low | 🟢 P2 | ✅ Fallback manual update |

---

## 🚀 Implementation Roadmap (Updated with Communication Strategy)

### Phase 1: Foundation + Native Messaging (Week 1-3)

**Goal:** Setup cross-platform infrastructure + Web-Desktop communication

**Tasks:**
1. ✅ Setup Tauri project với multi-platform targets
2. ✅ Define Domain Layer contracts (traits)
3. ✅ Implement printer discovery per platform
   - Windows: `WindowsPrinterRepository` (Win32 API)
   - macOS/Linux: `CupsPrinterRepository` (CUPS)
4. ✅ **Implement Native Messaging host**
   - Register manifest (Windows Registry / macOS plist / Linux config)
   - STDIN/STDOUT message loop
   - Handle: ping, print_batch, get_status
5. ✅ Unit tests cho printer discovery + Native Messaging
6. ✅ CI/CD matrix build (Windows/macOS/Linux)

**Deliverable:** 
- App có thể list printers trên cả 3 platforms
- Web app có thể gọi desktop app qua Native Messaging
- User click "In" → Desktop app nhận command → Print

**Communication:** Native Messaging ONLY (no WebSocket yet)

---

### Phase 2: MuPDF Integration + Hybrid Strategy (Week 4-5)

**Goal:** PDF rendering với Direct PDF + Render fallback

**Tasks:**
1. ✅ Integrate MuPDF library (compile cho 3 platforms)
2. ✅ Implement `DirectPdfStrategy` (send raw PDF to printer)
3. ✅ Implement `MuPdfRenderStrategy` với color conversion
   - RGB ↔ BGR
   - RGB → Grayscale
   - RGB → Binary
4. ✅ Strategy selector (auto-choose based on printer capability + config)
5. ✅ Margins application logic
6. ✅ Unit tests cho color conversion + strategy selection
7. ✅ Benchmark tests cho rendering performance

**Deliverable:** Render PDF với Hybrid Strategy (fast when possible, control when needed)

---

### Phase 3: Printer Config Management (Week 6)

**Goal:** Full config UI + persistence

**Tasks:**
1. ✅ Implement config UI (Tauri frontend)
   - Dropdown printer list
   - Color mode selector
   - Paper size + custom dimensions
   - Margin inputs (mm)
2. ✅ Config persistence (SQLite)
3. ✅ Units conversion logic (mm ↔ platform units)
4. ✅ Config validation
5. ✅ Integration tests với mock printer

**Deliverable:** User có thể configure printer và save config

---

### Phase 4: Queue & Job Processing (Week 7-8)

**Goal:** Batch printing workflow với Native Messaging

**Tasks:**
1. ✅ Implement Queue Worker
2. ✅ Batch processing logic (50 đơn/batch)
3. ✅ Parallel rendering với thread pool
4. ✅ Retry logic (max 3 attempts)
5. ✅ Status polling từ web app (2s interval)
6. ✅ Event-driven architecture
7. ✅ Integration tests với real workflow

**Deliverable:** Print 50 PDFs trong một batch với retry, web app poll status

---

### Phase 5: Auto-Update (Week 9)

**Goal:** Seamless updates trên 3 platforms

**Tasks:**
1. ✅ Configure Tauri updater plugin
2. ✅ Setup release server (https://releases.sapo.vn)
3. ✅ Implement update check UI
4. ✅ Config backup/restore logic
5. ✅ Platform-specific installers
   - Windows: NSIS
   - macOS: DMG + code signing
   - Linux: AppImage
6. ✅ Update testing trên 3 platforms

**Deliverable:** App tự động update khi có version mới

---

### Phase 6: Testing & Polish (Week 10-11)

**Goal:** Production-ready quality

**Tasks:**
1. ✅ E2E tests với real printers
2. ✅ Visual regression tests (screenshot comparison)
3. ✅ Performance benchmarks (meet SLA: 50 PDFs < 60s)
4. ✅ Error handling polish
5. ✅ User documentation
   - Windows setup guide
   - macOS setup guide
   - Linux setup guide (CUPS config)
6. ✅ Accessibility audit (keyboard navigation, screen readers)

**Deliverable:** Production-ready app với full documentation

---

### **Phase 7 (Optional): Add WebSocket** (Week 12 - When needed)

**Triggers to start Phase 7:**
- ✅ User feedback: "Status không realtime, phải refresh"
- ✅ Requirement: In từ mobile app
- ✅ Requirement: Admin dashboard xem tất cả máy in realtime
- ✅ Requirement: Scheduled/offline printing (web app đóng vẫn in được)

**Goal:** Realtime updates + Server-push capabilities

**Tasks:**
1. ✅ Setup WebSocket server infrastructure
   - `wss://api.sapo.vn/printer/v1/ws`
   - Authentication với Device Token
   - Connection pool management
2. ✅ Desktop app WebSocket client
   - Connect on startup
   - Auto-reconnect logic
   - Handle server push events
3. ✅ Web app WebSocket client (optional - cho realtime updates)
4. ✅ Server-side job queue
   - Push jobs to desktop via WebSocket
   - Receive status updates from desktop
5. ✅ Integration tests cho WebSocket flow

**Deliverable:** 
- Realtime status updates (no polling)
- Multi-tab sync
- Offline printing capability
- Foundation cho mobile app + admin dashboard

**Estimated effort:** +1 week

---

## 📦 Deployment Strategy

### Build Artifacts per Platform

| Platform | Installer Type | Size (approx) | Distribution |
|----------|---------------|---------------|--------------|
| Windows | NSIS `.exe` | ~50MB | Direct download + auto-update |
| macOS | DMG + `.app` | ~60MB | Direct download + Sparkle update |
| Linux | AppImage | ~65MB | Direct download + manual update |

### Release Process

```bash
# 1. Version bump
cargo bump patch  # or minor/major

# 2. Build all platforms (CI)
npm run tauri build -- --target x86_64-pc-windows-msvc
npm run tauri build -- --target x86_64-apple-darwin
npm run tauri build -- --target x86_64-unknown-linux-gnu

# 3. Sign artifacts
# Windows: signtool.exe
# macOS: codesign + notarization
# Linux: GPG signature

# 4. Upload to release server
aws s3 cp ./bundle/ s3://releases.sapo.vn/printer-client/ --recursive

# 5. Update manifest
cat > latest.json <<EOF
{
  "version": "1.0.0",
  "notes": "Bug fixes and performance improvements",
  "pub_date": "2026-06-22T10:00:00Z",
  "platforms": {
    "windows": { "url": "https://..." },
    "darwin": { "url": "https://..." },
    "linux": { "url": "https://..." }
  }
}
EOF

# 6. Smoke test
./smoke-test.sh --all-platforms
```

---

## 🎯 Success Metrics

### Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| **Printer discovery** | < 2s | Time to list all printers |
| **PDF rendering** | < 1s per page | MuPDF render time (300 DPI) |
| **Batch processing** | < 60s for 50 PDFs | End-to-end (download → print) |
| **Memory usage** | < 500MB | Peak during batch processing |
| **App startup** | < 3s | Cold start to UI ready |

### Quality Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| **Test coverage** | > 80% | Code coverage (domain + app layer) |
| **Cross-platform parity** | 100% features | Feature availability matrix |
| **Update success rate** | > 99% | Successful updates / total attempts |
| **Print success rate** | > 99.5% | Successful prints / total jobs |
| **Crash rate** | < 0.1% | Crashes per 1000 sessions |

---

## 🔧 Tech Stack Summary

### Core Technologies

| Layer | Technology | Rationale |
|-------|-----------|-----------|
| **Framework** | Tauri 1.x | Cross-platform, small bundle size |
| **Language** | Rust | Performance, safety, cross-platform |
| **Frontend** | React + TypeScript | Modern UI, type safety |
| **PDF Rendering** | MuPDF | High performance, cross-platform |
| **Database** | SQLite | Embedded, cross-platform |
| **Printer API** | Win32 + CUPS | Native OS integration |
| **Auto-update** | Tauri Updater | Built-in, multi-platform |

### Key Dependencies

```toml
# Cargo.toml
[dependencies]
tauri = "1.5"
mupdf-sys = "0.3"      # MuPDF FFI bindings
sqlx = "0.7"           # SQLite async
tokio = "1.35"         # Async runtime
rayon = "1.8"          # Parallel processing
serde = "1.0"          # Serialization
tracing = "0.1"        # Logging

# Platform-specific
[target.'cfg(windows)'.dependencies]
winapi = "0.3"         # Win32 API

[target.'cfg(not(windows))'.dependencies]
cups-sys = "0.2"       # CUPS FFI (macOS/Linux)
```

---

## 📚 References

### Documentation
- [Tauri Documentation](https://tauri.app/)
- [MuPDF Documentation](https://mupdf.readthedocs.io/)
- [CUPS API Reference](https://www.cups.org/doc/api-cups.html)
- [Win32 Printing API](https://learn.microsoft.com/en-us/windows/win32/printdocs/printing-and-print-spooler)
- [Chrome Native Messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging)

### Related Files
- `docs/srs-in.md` — Architecture specification (DDD + Clean Architecture)
- `docs/SRS_In số lượng lớn.md` — Detailed SRS for bulk printing (387KB)
- `_bmad/config.toml` — BMad framework configuration

### Competitor Analysis
- **BigSeller Print Plugin** — Native Messaging + Remote API pattern
- Flow: Web App → Browser Native Messaging → Desktop App → WebSocket → Server
- Authentication: Device registration via remote endpoint
- Screenshots: `docs/images/Screenshot 2026-06-22 091805.png`, `docs/images/Screenshot 2026-06-22 091907.png`

---

## 🔌 **Web-Desktop Communication Strategy**

### **Pattern Analysis: BigSeller Approach**

**BigSeller uses:**
1. **Native Messaging** (Browser ↔ Desktop) — For web app commands
2. **Remote API Connection** (Desktop → Server) — For job fetching & status updates
3. **No Browser Extension** — Desktop app registers as Native Messaging host

**Key insights:**
- Desktop app listens on `https://www.bigseller.com/api/v1/print/plugin`
- "Check Connection" button in System Settings
- Real-time logs: Version, Printing, Connection status
- UI config: Printer, Paper (10cm×15cm), Margins (L/R/T/B), Color Mode (RGB)

---

## 🏗️ **SAPO Communication Architecture (Phased)**

### **Phase 1: Native Messaging ONLY** (MVP - Week 1-3)

**Simple, proven pattern:**

```
┌─────────────┐         ┌──────────────┐         ┌─────────────┐
│  SAPO Web   │         │   Browser    │         │  Desktop    │
│  (React)    │         │              │         │  App        │
└──────┬──────┘         └───────┬──────┘         └──────┬──────┘
       │                        │                        │
       │ 1. User click "In"     │                        │
       ├───────────────────────►│                        │
       │    chrome.runtime      │  2. Native Messaging   │
       │    .sendNativeMessage  ├───────────────────────►│
       │                        │    STDIN/STDOUT IPC    │
       │                        │                        │
       │                        │  3. Response           │
       │                        │◄───────────────────────┤
       │ 4. Callback            │    {job_id, status}    │
       │◄───────────────────────┤                        │
       │                        │                        │
       │ 5. Poll status         │                        │
       ├───────────────────────►├───────────────────────►│
       │    (every 2s)          │                        │
```

**Implementation:**

```javascript
// Web App (SAPO React)
async function printBulk(pdfUrls, config) {
  // Check if desktop app is installed
  try {
    const pingResponse = await chrome.runtime.sendNativeMessage(
      'com.sapo.printer',
      { action: 'ping' }
    );
    console.log('Desktop app version:', pingResponse.version);
  } catch (error) {
    alert('Vui lòng cài đặt SAPO Printer Desktop App');
    return;
  }
  
  // Start print job
  const response = await chrome.runtime.sendNativeMessage(
    'com.sapo.printer',
    {
      action: 'print_batch',
      pdf_urls: pdfUrls,
      printer: config.printer,
      config: {
        paper_size: config.paper_size,
        margins: config.margins,
        color_mode: config.color_mode
      }
    }
  );
  
  if (!response.success) {
    alert('Lỗi: ' + response.error);
    return;
  }
  
  // Poll status
  const jobId = response.job_id;
  const intervalId = setInterval(async () => {
    const statusResponse = await chrome.runtime.sendNativeMessage(
      'com.sapo.printer',
      { action: 'get_status', job_id: jobId }
    );
    
    updateProgressUI(statusResponse.status);
    
    if (statusResponse.status.state === 'COMPLETED' || 
        statusResponse.status.state === 'FAILED') {
      clearInterval(intervalId);
      showResult(statusResponse.status);
    }
  }, 2000); // Poll every 2 seconds
}
```

```rust
// Desktop App (Tauri/Rust)
use std::io::{self, Read, Write};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct NativeMessage {
    action: String,
    job_id: Option<String>,
    pdf_urls: Option<Vec<String>>,
    printer: Option<String>,
    config: Option<PrintConfig>,
}

#[derive(Serialize)]
struct NativeResponse {
    success: bool,
    job_id: Option<String>,
    version: Option<String>,
    status: Option<JobStatus>,
    error: Option<String>,
}

fn main() {
    // Register as Native Messaging host on startup
    register_native_host().unwrap();
    
    // Native Messaging loop
    loop {
        match read_native_message() {
            Ok(msg) => {
                let response = handle_message(msg);
                write_native_message(&response).unwrap();
            }
            Err(e) => {
                eprintln!("Error reading message: {}", e);
                break;
            }
        }
    }
}

fn handle_message(msg: NativeMessage) -> NativeResponse {
    match msg.action.as_str() {
        "ping" => NativeResponse {
            success: true,
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            ..Default::default()
        },
        
        "print_batch" => {
            let job_id = uuid::Uuid::new_v4().to_string();
            
            // Create print job async
            tokio::spawn(async move {
                let use_case = CreatePrintJobUseCase::new(/* ... */);
                use_case.execute(PrintJobRequest {
                    pdf_urls: msg.pdf_urls.unwrap(),
                    printer: msg.printer.unwrap(),
                    config: msg.config.unwrap(),
                }).await.unwrap();
            });
            
            NativeResponse {
                success: true,
                job_id: Some(job_id),
                ..Default::default()
            }
        },
        
        "get_status" => {
            let status = get_job_status(&msg.job_id.unwrap()).unwrap();
            NativeResponse {
                success: true,
                status: Some(status),
                ..Default::default()
            }
        },
        
        _ => NativeResponse {
            success: false,
            error: Some("Unknown action".to_string()),
            ..Default::default()
        }
    }
}

fn register_native_host() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    {
        use winreg::RegKey;
        let hkcu = RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let path = r"Software\Google\Chrome\NativeMessagingHosts\com.sapo.printer";
        let (key, _) = hkcu.create_subkey(path)?;
        
        let manifest_path = get_manifest_path()?;
        key.set_value("", &manifest_path)?;
    }
    
    Ok(())
}
```

**Pros:**
- ✅ Simple implementation (1-2 weeks)
- ✅ No server infrastructure needed
- ✅ No firewall issues (IPC, not network)
- ✅ Proven by BigSeller

**Cons:**
- ⚠️ Web app must poll for status (2s interval acceptable)
- ⚠️ No multi-tab sync
- ⚠️ No offline printing

---

### **Phase 2: Add WebSocket** (When needed - Week 8-9)

**Triggers to add WebSocket:**
- User feedback: "Status không update realtime"
- Requirement: In từ mobile app
- Requirement: Admin dashboard xem tất cả máy in
- Requirement: Scheduled/offline printing

**Enhanced architecture:**

```
┌─────────────┐         ┌──────────────┐         ┌─────────────┐
│  SAPO Web   │         │   Browser    │         │  Desktop    │
└──────┬──────┘         └───────┬──────┘         └──────┬──────┘
       │                        │                        │
       │ Native Messaging       │                        │
       │◄──────────────────────►│◄──────────────────────►│
       │  (Print commands)      │                        │
       │                        │                        │
       │                        │      WebSocket         │
       │                        │          ↕              │
       │◄───────────────────────┼──────────┴──────────────┤
       │   Server Push          │    wss://api.sapo.vn   │
       │   (Realtime updates)   │                        │
```

**WebSocket additions:**

```rust
// Desktop App - Add WebSocket client
#[tokio::main]
async fn main() {
    // Start Native Messaging in background
    tokio::spawn(async {
        native_messaging_loop();
    });
    
    // Connect to server via WebSocket
    let ws_client = WebSocketClient::connect(
        "wss://api.sapo.vn/printer/v1/ws",
        &device_token
    ).await.unwrap();
    
    // Listen for server events
    loop {
        match ws_client.recv().await {
            ServerEvent::PrintJobCreated(job) => {
                // Server can push jobs even when web closed
                create_print_job_from_server(job).await.unwrap();
            }
            
            ServerEvent::CancelJob(job_id) => {
                cancel_print_job(job_id).await.unwrap();
            }
            
            _ => {}
        }
    }
}

// When job status changes, push to server
async fn update_job_status(job_id: String, status: JobStatus) {
    // Update local DB
    update_local_status(&job_id, &status).await.unwrap();
    
    // Push to server via WebSocket
    ws_client.send(ClientEvent::JobStatusUpdate {
        job_id,
        status
    }).await.unwrap();
}
```

**Benefits of Phase 2:**
- ✅ Realtime updates (no polling)
- ✅ Multi-tab sync
- ✅ Offline printing (server push to desktop)
- ✅ Mobile app support
- ✅ Admin dashboard realtime

**Cost:**
- ⚠️ +1 week development
- ⚠️ WebSocket server infrastructure
- ⚠️ Connection management complexity

---

## 🎯 **Decision Matrix**

| Feature | Phase 1 (Native Messaging) | Phase 2 (+ WebSocket) |
|---------|----------------------------|----------------------|
| **Web → Desktop print** | ✅ Direct | ✅ Direct |
| **Status updates** | ⚠️ Poll (2s) | ✅ Realtime push |
| **Multi-tab sync** | ❌ | ✅ |
| **Offline printing** | ❌ | ✅ |
| **Mobile app support** | ❌ | ✅ |
| **Admin dashboard** | ❌ | ✅ |
| **Implementation time** | 2 weeks | +1 week |
| **Infrastructure** | None | WebSocket server |
| **Complexity** | Low | Medium |

---

## ✅ Key Takeaways

### Critical Success Factors

1. **MuPDF over PDFium** — 2-3x performance improvement critical for bulk printing
2. **Platform-specific abstractions** — Clean separation via Repository pattern
3. **Units normalization** — Internal mm, convert to platform units when needed
4. **Color mode strategy** — Support 5 modes with robust conversion pipeline
5. **Parallel rendering** — CPU-bound task, must leverage multi-core
6. **Graceful degradation** — Validate, timeout, fallback on errors
7. **Cross-platform CI** — Test matrix prevents platform-specific regressions

### Architectural Decisions

1. **Clean Architecture + DDD** — Domain layer 100% platform-agnostic
2. **Event-Driven** — All state changes emit domain events
3. **Strategy Pattern** — Printer discovery + rendering per platform
4. **Repository Pattern** — Data access abstraction
5. **Use Case Pattern** — Application layer orchestration

### Risk Mitigation

1. **Printer detection** — Healthcheck UI + clear error messages
2. **PDF validation** — Catch corrupt PDFs before rendering
3. **Color accuracy** — Platform-specific unit tests
4. **Performance** — Benchmarks in CI, parallel processing
5. **Updates** — Fallback to manual update on Linux AppImage

---

**Document Status:** ✅ Complete  
**Next Steps:** Phase 1 Implementation — Printer Discovery Foundation  
**Estimated Timeline:** 10 weeks to production-ready

