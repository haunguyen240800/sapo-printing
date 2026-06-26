# Bản Thiết Kế Chi Tiết: Triển Khai Printing Pipeline (Chuẩn DDD)

Bản kế hoạch này bám sát **100% thiết kế từ tài liệu `pipeline-in.md`**, đồng thời làm rõ chi tiết cách code (implementation) bằng ngôn ngữ Rust sao cho chuẩn DDD (tránh lạm dụng macro, sử dụng Dependency Injection và Event Sourcing).

---

## 1. Nguyên Tắc Coding (DDD Coding Rules)
1. **Hướng tâm (Dependency Rule):** Infrastructure phụ thuộc Application, Application phụ thuộc Domain.
2. **Explicit (Tường minh):** Tuân thủ triết lý Rust, các Aggregate Root (như `PrintJob`) phải tự khai báo mảng `domain_events` rõ ràng, không dùng phép thuật che giấu.
3. **Mọi tương tác ra ngoài** đều phải thông qua **Trait (Interface)**.

---

## 2. Thiết Kế Các Base DDD (Common Domain Patterns)

Chuyển thể các abstract class/interface từ Java (thư mục `docs/example/`) sang chuẩn Rust (Traits). Đặt tại `src-tauri/src/domain/common/`.

**1. Exception/Error (`error.rs`)**
```rust
use std::fmt;

#[derive(Debug, Clone)]
pub struct DomainValidationException {
    pub message: String,
}

impl DomainValidationException {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }
}

impl fmt::Display for DomainValidationException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Domain Validation Error: {}", self.message)
    }
}

impl std::error::Error for DomainValidationException {}
```

**2. Domain Rule (`rule.rs`)**
```rust
pub trait DomainRule {
    fn is_broken(&self) -> bool;
    fn message(&self) -> String;
}
```

**3. Value Object (`value_object.rs`)**
```rust
use super::rule::DomainRule;
use super::error::DomainValidationException;

pub trait ValueObject: PartialEq {
    fn check_rule(&self, rule: &dyn DomainRule) -> Result<(), DomainValidationException> {
        if rule.is_broken() {
            Err(DomainValidationException::new(rule.message()))
        } else {
            Ok(())
        }
    }
}
```

**4. Nested Domain Entity (`entity.rs`)**
```rust
pub trait NestedDomainEntity {
    fn id(&self) -> &str;
}
```

**5. Aggregate Root & Domain Event (`aggregate.rs`)**
```rust
pub trait DomainEvent {
    fn event_name(&self) -> &'static str;
}

pub trait AggregateRoot {
    fn domain_events(&self) -> &[Box<dyn DomainEvent>];
    fn clear_domain_events(&mut self);
}
```

---

## Layer 1 - Print Queue & Domain Models

**Nhiệm vụ:** Quản lý hàng đợi, Retry, Pause, Cancel. Hoàn toàn không thực hiện render.

Trong chuẩn DDD, `PrintJob` là một **Aggregate Root**. Chúng ta sẽ khai báo chính xác các thuộc tính theo thiết kế và thêm mảng chứa Domain Event.

```rust
// src-tauri/src/domain/print_job/mod.rs
use crate::domain::common::aggregate::{AggregateRoot, DomainEvent};

pub struct PrintJob {
    pub id: String, // JobId
    pub pdf_path: String, // PathBuf
    pub printer_id: String, // PrinterId
    pub settings: PrintSettings,
    
    // Quản lý trạng thái và sự kiện DDD
    pub status: String,
    domain_events: Vec<Box<dyn DomainEvent>>, 
}

impl AggregateRoot for PrintJob {
    fn domain_events(&self) -> &[Box<dyn DomainEvent>] { &self.domain_events }
    fn clear_domain_events(&mut self) { self.domain_events.clear(); }
}

impl PrintJob {
    pub fn mark_printing(&mut self) {
        self.status = "PRINTING".to_string();
        self.domain_events.push(Box::new(PrintJobPrintingEvent { id: self.id.clone() }));
    }
}
```

---

## Layer 2 - Print Service (Application Layer)

**Nhiệm vụ:** Điều phối toàn bộ quá trình: Load Job -> Load PDF -> Layout -> Render -> Print.

Đây là một Application Service chuẩn DDD. Chúng ta định nghĩa Interface `PrintService` và tạo Struct `DefaultPrintService` (có thể đổi tên tùy ý) để tiêm Dependencies vào.

```rust
// src-tauri/src/application/services/print_service.rs
pub trait PrintService {
    fn execute(&self, job: PrintJob) -> Result<(), String>;
}

pub struct DefaultPrintService {
    job_repo: Arc<dyn IJobRepository>,
    event_bus: Arc<dyn IEventBus>,
    downloader: Arc<dyn IPdfDownloader>,
}

impl PrintService for DefaultPrintService {
    fn execute(&self, mut job: PrintJob) -> Result<(), String> {
        let pdf_path = self.downloader.download(&job.pdf_path)?;
        
        let (pdf_w, pdf_h) = get_pdf_size(&pdf_path); 
        let transform = LayoutEngine::calculate(&job.settings, pdf_w, pdf_h);

        job.mark_printing();
        self.persist_and_publish(&mut job)?; // Lưu DB & Gửi WebSocket
        
        let mut backend = GraphicsBackendFactory::create();
        backend.begin_document(&job.settings.printer_name, "Sapo Order")?;
        
        // Chọn Strategy lúc runtime
        let strategy: Box<dyn RenderStrategy> = if job.settings.binary {
            Box::new(BitmapRenderStrategy::new())
        } else {
            Box::new(NativePdfRenderStrategy::new())
        };
        
        strategy.render(&pdf_path, &transform, &job.settings, &mut *backend);
        
        backend.end_document();
        job.mark_completed();
        self.persist_and_publish(&mut job)?;
        
        Ok(())
    }
}
```

---

## Layer 3 - Layout Engine (Application Layer)

**Nhiệm vụ:** Tính toán layout trước khi render. Mọi phép toán đều độc lập với UI.

```rust
// src-tauri/src/domain/layout/mod.rs
#[derive(Debug, Clone, PartialEq)]
pub struct Transform {
    pub scale_x: f32,
    pub scale_y: f32,
    pub translate_x: f32,
    pub translate_y: f32,
    pub rotation: f32,
}
impl crate::domain::common::value_object::ValueObject for Transform {}

// src-tauri/src/application/services/layout_engine.rs
pub struct LayoutEngine;
impl LayoutEngine {
    pub fn calculate(settings: &PrintSettings, pdf_width: f32, pdf_height: f32) -> Transform {
        // Toán học tính toán Scale, Offset và Rotation dựa vào settings.paper_size và margins.
        Transform { scale_x: 1.0, scale_y: 1.0, translate_x: 0.0, translate_y: 0.0, rotation: settings.rotate }
    }
}
```

---

## Layer 4 - Print Settings (Domain Layer)

**Nhiệm vụ:** Lưu toàn bộ cấu hình in. Nó là một **Value Object**.

```rust
// src-tauri/src/domain/settings/mod.rs
#[derive(Debug, Clone, PartialEq)]
pub struct PrintSettings {
    pub paper_size: String,
    pub orientation: String,
    pub margin: f32,
    pub scale_mode: String,
    pub dpi: u32,
    pub grayscale: bool,
    pub binary: bool,
    pub copies: u32,
    pub rotate: f32,
}
// Implement chuẩn ValueObject
impl crate::domain::common::value_object::ValueObject for PrintSettings {}
```

---

## Layer 5 - PDF Renderer (Infrastructure Layer)

**Nhiệm vụ:** Adapter giữa Domain và PDFium. Tuyệt đối không rò rỉ (leak) con trỏ C++ của PDFium lên Domain.

```rust
// src-tauri/src/infrastructure/pdfium/renderer.rs
pub trait PdfRenderer {
    // PdfPage là một wrapper an toàn, che giấu con trỏ FPDF_PAGE
    fn render_page(
        &self,
        page: &PdfPage,
        transform: &Transform,
        backend: &mut dyn GraphicsBackend,
    );
}
```

---

## Layer 6 - Render Strategy (Infrastructure Layer)

**Nhiệm vụ:** Cho phép thay đổi chiến lược render tuỳ thuộc vào máy in/hệ điều hành.

```rust
pub trait RenderStrategy {
    fn render(&self, pdf_path: &str, transform: &Transform, settings: &PrintSettings, backend: &mut dyn GraphicsBackend);
}
```

1. **`BitmapRenderStrategy`:** Dùng `pdfium-render` xuất ra mảng Byte ảnh (Bitmap), sau đó gọi `backend.draw_bitmap()`. An toàn đa nền tảng nhưng Spooler bị lớn.
2. **`NativePdfRenderStrategy`:** Dùng FFI (`FPDF_RenderPage`). Trích xuất `NativeGraphicsContext` từ Backend và truyền thẳng vào PDFium. Spooler nhỏ, giữ nguyên vector, chữ siêu nét (chuẩn Chrome).
3. **Mở rộng tương lai:** Sẵn sàng cho `ZplRenderStrategy`, `TsplRenderStrategy`.

---

## Layer 7 - Graphics Backend (Infrastructure Layer)

**Nhiệm vụ:** Abstraction giữa PDF Renderer và Hệ điều hành. Không dính dáng đến parse hay render PDF.

```rust
// src-tauri/src/infrastructure/graphics/backend.rs
pub enum NativeGraphicsContext {
    Windows(windef::HDC),
    Mac(core_graphics::CGContextRef),
    Linux(cairo::cairo_t),
}

pub trait GraphicsBackend {
    fn begin_document(&mut self, printer_name: &str, doc_name: &str) -> Result<(), String>;
    fn begin_page(&mut self);
    
    // Quan trọng: Phục vụ cho NativePdfRenderStrategy
    fn native_context(&mut self) -> NativeGraphicsContext;
    
    // Quan trọng: Phục vụ cho BitmapRenderStrategy
    fn draw_bitmap(&mut self, data: &[u8], width: u32, height: u32, bpp: u16);
    
    fn end_page(&mut self);
    fn end_document(&mut self);
}
```

**Triển khai thực tế trên từng OS (thông qua `GraphicsBackendFactory`):**
- **Windows (`windows.rs`):** Cấp phát `HDC` qua Windows GDI (`CreateDCW`, `StartDocW`). `native_context` trả về `NativeGraphicsContext::Windows(hdc)`.
- **macOS (`macos.rs`):** Tạo `CGContext` qua Quartz. `native_context` trả về `Mac(CGContextRef)`. `end_document` đẩy file sang CUPS/Printer Driver.
- **Linux (`linux.rs`):** Tạo context qua Cairo. `native_context` trả về `Linux(cairo_t)`. Đẩy sang CUPS.

---

## Cấu trúc thư mục

```text
src-tauri/src/
├── application/
│   ├── usecases/
│   ├── services/       <-- print_service.rs (Chứa Trait PrintService & DefaultPrintService), LayoutEngine
│   └── dto/
├── domain/
│   ├── print_job/      <-- PrintJob (Aggregate Root)
│   ├── settings/       <-- PrintSettings (Value Object)
│   ├── layout/         <-- Transform (Value Object)
│   └── common/         <-- AggregateRoot, ValueObject, DomainRule, NestedDomainEntity, DomainValidationException
├── infrastructure/
│   ├── pdfium/         <-- renderer.rs, bitmap_strategy.rs, native_strategy.rs
│   ├── graphics/       <-- backend.rs, windows.rs, macos.rs, linux.rs
│   ├── printer/
│   └── queue/          <-- QueueWorker (Đã được làm sạch, gọi PrintService)
└── presentation/
```
