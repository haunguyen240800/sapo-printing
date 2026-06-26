// Using raw types (or u64) as placeholders for FFI pointers to avoid bringing heavy OS dependencies in this mock
pub enum NativeGraphicsContext {
    Windows(usize), // HDC
    Mac(usize),     // CGContextRef
    Linux(usize),   // cairo_t
}

pub trait GraphicsBackend {
    fn begin_document(&mut self, printer_name: &str, doc_name: &str) -> Result<(), String>;
    fn begin_page(&mut self);
    fn native_context(&mut self) -> NativeGraphicsContext;
    fn draw_bitmap(&mut self, data: &[u8], width: u32, height: u32, bpp: u16);
    fn end_page(&mut self);
    fn end_document(&mut self);
}

pub struct GraphicsBackendFactory;

impl GraphicsBackendFactory {
    pub fn create() -> Box<dyn GraphicsBackend> {
        #[cfg(target_os = "windows")]
        {
            Box::new(super::windows::WindowsGraphicsBackend::new())
        }
        #[cfg(target_os = "macos")]
        {
            Box::new(super::macos::MacGraphicsBackend::new())
        }
        #[cfg(target_os = "linux")]
        {
            Box::new(super::linux::LinuxGraphicsBackend::new())
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            unimplemented!("Unsupported platform");
        }
    }
}
