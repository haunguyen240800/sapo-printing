# Printing Pipeline sử dụng PDFium

## Mục tiêu

Xây dựng một Printing Pipeline có kiến trúc tương tự Chrome nhưng phù hợp với Rust + Tauri + DDD, đảm bảo:

- Tách biệt Domain và Infrastructure.
- Dễ thay thế PDFium bằng MuPDF hoặc engine khác.
- Hỗ trợ đa nền tảng (Windows, macOS, Linux).
- Hỗ trợ nhiều chiến lược render (Native, Bitmap).
- Dễ mở rộng thêm các loại máy in (Laser, Thermal, Label Printer...).

---

# Kiến trúc tổng thể

```text
                        Website
                           │
                  HTTP / WebSocket
                           │
                           ▼
                   Rust + Tauri Desktop
                           │
            ┌──────────────┴──────────────┐
            ▼                             ▼
      Print Queue Service          Printer Manager
            │                             │
            └──────────────┬──────────────┘
                           ▼
                    Print Pipeline
                           │
        ┌──────────────────┼──────────────────┐
        ▼                  ▼                  ▼
   Layout Engine      PDF Renderer      Print Settings
                           │
                           ▼
                   Render Strategy
                           │
          ┌────────────────┴────────────────┐
          ▼                                 ▼
 Bitmap Strategy                 Native Strategy
(pdfium-render)             (PDFium Native API)
          │                                 │
          └────────────────┬────────────────┘
                           ▼
                  Graphics Backend
          ┌────────────────┼────────────────┐
          ▼                ▼                ▼
     Windows GDI      macOS Quartz      Linux CUPS
                           │
                           ▼
                     Printer Driver
                           │
                           ▼
                         Printer
```

---

# Layer 1 - Print Queue

Chịu trách nhiệm:

- Quản lý hàng đợi.
- Retry.
- Pause.
- Resume.
- Cancel.
- Theo dõi trạng thái.

Không thực hiện render.

Ví dụ:

```rust
PrintJob
```

```rust
pub struct PrintJob {
    id: JobId,
    pdf_path: PathBuf,
    printer_id: PrinterId,
    settings: PrintSettings,
}
```

---

# Layer 2 - Print Pipeline

Điều phối toàn bộ quá trình in.

```text
Load Job
    │
    ▼
Load PDF
    │
    ▼
Layout
    │
    ▼
Render
    │
    ▼
Print
```

Interface

```rust
pub trait PrintPipeline {
    fn execute(job: PrintJob) -> Result<()>;
}
```

---

# Layer 3 - Layout Engine

Chịu trách nhiệm tính toán layout trước khi render.

Input:

- Paper Size
- Margin
- Scale
- Orientation
- Rotation

Output:

```rust
pub struct Transform {
    scale_x: f32,
    scale_y: f32,
    translate_x: f32,
    translate_y: f32,
    rotation: f32,
}
```

Ví dụ

```text
PDF:      98 × 148 mm
Paper:   100 × 150 mm
```

↓

```text
Scale = 1.013
OffsetX = 1 mm
OffsetY = 1 mm
```

---

# Layer 4 - Print Settings

Lưu toàn bộ cấu hình in.

```rust
pub struct PrintSettings {

    paper_size: PaperSize,

    orientation: Orientation,

    margin: Margin,

    scale_mode: ScaleMode,

    dpi: u32,

    grayscale: bool,

    binary: bool,

    copies: u32,

    rotate: Rotation,

}
```

Ví dụ:

```text
Paper: 100 × 150

Margin Left: 2 mm

Margin Top: 3 mm

Scale: Fit

Binary: true

DPI: 300
```

---

# Layer 5 - PDF Renderer

Đây là Adapter giữa Domain và PDFium.

```rust
pub trait PdfRenderer {

    fn render_page(
        page: PdfPage,
        transform: Transform,
        backend: &mut dyn GraphicsBackend,
    );

}
```

Không phụ thuộc vào:

- Windows
- macOS
- Linux

Chỉ biết:

- PDF
- Transform
- Graphics Backend

---

# Layer 6 - Render Strategy

Cho phép thay đổi chiến lược render.

```rust
pub trait RenderStrategy {

    fn render(...);

}
```

Có thể triển khai:

## BitmapRenderStrategy

```text
PDF

↓

Bitmap

↓

Graphics Backend
```

Sử dụng:

```text
pdfium-render
```

Ưu điểm

- Cross Platform
- Đơn giản
- Dễ triển khai

Nhược điểm

- File spool lớn
- Chữ có thể bị rasterize

---

## NativePdfRenderStrategy

```text
PDF

↓

FPDF_RenderPage()

↓

Graphics Context
```

Ưu điểm

- Giữ vector
- Giữ text
- Spool nhỏ
- Chất lượng gần Chrome

Nhược điểm

- Cần FFI
- Phức tạp hơn

---

# Layer 7 - Graphics Backend

Đây là abstraction giữa PDF Renderer và hệ điều hành.

```rust
pub trait GraphicsBackend {

    fn begin_document(&mut self);

    fn begin_page(&mut self);

    fn native_context(&mut self)
        -> NativeGraphicsContext;

    fn end_page(&mut self);

    fn end_document(&mut self);

}
```

Không render PDF.

Không parse PDF.

Chỉ cung cấp Graphics Context cho PDFium.

---

# Platform Backend

## Windows

```text
PDFium

↓

HDC

↓

Windows GDI

↓

Printer Driver

↓

Printer
```

Native Context

```text
HDC
```

---

## macOS

```text
PDFium

↓

CGContext

↓

Quartz

↓

Printer Driver
```

Native Context

```text
CGContextRef
```

---

## Linux

```text
PDFium

↓

Cairo

↓

CUPS

↓

Printer Driver
```

Native Context

```text
cairo_t
```

---

# Graphics Context

Không expose API GDI trực tiếp lên Domain.

Ví dụ

```rust
enum NativeGraphicsContext {

    Windows(HDC),

    Mac(CGContextRef),

    Linux(CairoContext),

}
```

PDFium chỉ cần lấy Native Context phù hợp.

---

# Quy trình in

```text
Print Job

↓

Load PDF

↓

Load Print Settings

↓

Layout Engine

↓

Transform Matrix

↓

Render Strategy

↓

Graphics Backend

↓

Printer Driver

↓

Printer
```

---

# Cấu trúc thư mục đề xuất

```text
src/

├── application/
│
│   ├── usecases/
│   ├── services/
│   └── dto/
│
├── domain/
│
│   ├── print_job/
│   ├── printer/
│   ├── queue/
│   ├── settings/
│   └── repository/
│
├── infrastructure/
│
│   ├── pdfium/
│   │      ├── renderer.rs
│   │      ├── bitmap_strategy.rs
│   │      └── native_strategy.rs
│   │
│   ├── graphics/
│   │      ├── backend.rs
│   │      ├── windows.rs
│   │      ├── macos.rs
│   │      └── linux.rs
│   │
│   ├── printer/
│   └── queue/
│
└── presentation/
```

---

# Luồng hoạt động

```text
Website

↓

Desktop

↓

Create Print Job

↓

Queue

↓

Print Pipeline

↓

Layout Engine

↓

Transform Matrix

↓

Render Strategy

↓

PDFium

↓

Graphics Backend

↓

Printer Driver

↓

Printer
```

---

# Hướng mở rộng

Có thể bổ sung thêm các Render Strategy khác:

```text
Render Strategy

├── BitmapRenderStrategy

├── NativePdfRenderStrategy

├── ZplRenderStrategy

├── TsplRenderStrategy

├── EscPosRenderStrategy

└── ImageRenderStrategy
```

Nhờ đó hệ thống có thể lựa chọn chiến lược in phù hợp theo:

- Loại máy in.
- Hệ điều hành.
- Định dạng tài liệu.
- Hiệu năng mong muốn.

---

# Ưu điểm của kiến trúc

- Tuân thủ DDD, Domain không phụ thuộc PDFium.
- Có thể thay thế PDFium bằng MuPDF mà không ảnh hưởng Application.
- Hỗ trợ đa nền tảng.
- Có thể lựa chọn giữa Bitmap và Native Rendering.
- Dễ mở rộng thêm các ngôn ngữ in như ZPL, TSPL, ESC/POS.
- Tách biệt rõ trách nhiệm giữa Queue, Layout, Render và Platform Backend.
- Phù hợp cho các hệ thống in hàng loạt, quản lý hàng đợi và nhiều loại máy in.