// Using raw types (or u64) as placeholders for FFI pointers to avoid bringing heavy OS dependencies in this mock
pub enum NativeGraphicsContext {
    Windows(usize), // HDC
    Mac(usize),     // CGContextRef
    Linux(usize),   // cairo_t
}

pub trait GraphicsBackend {
    /// `paper_width_mm` / `paper_height_mm` — physical paper dimensions in mm.
    /// Used on Windows to populate DEVMODEW so the spooler creates the DC at the
    /// correct page size instead of the driver's default (usually A4).
    fn begin_document(&mut self, printer_name: &str, doc_name: &str, output_path: Option<&str>, paper_width_mm: f32, paper_height_mm: f32) -> Result<(), String>;
    fn get_dpi(&self) -> (u32, u32);
    /// Returns the printable area in device pixels (HORZRES × VERTRES on Windows,
    /// estimated from paper_mm × DPI on other platforms).  Used to size the render
    /// bitmap so it fits exactly inside the DC without clipping.
    fn get_page_pixels(&self) -> (u32, u32);
    fn begin_page(&mut self);
    fn native_context(&mut self) -> NativeGraphicsContext;
    fn draw_bitmap(&mut self, data: &[u8], x: i32, y: i32, width: u32, height: u32, bpp: u16);
    fn end_page(&mut self);

    /// Finish the current document and submit it to the spooler, then block
    /// until the OS confirms the spooled job actually reached the printer.
    ///
    /// On Windows this polls the print spooler for the job created by
    /// `begin_document` until it reports `JOB_STATUS_PRINTED` (or vanishes from
    /// the queue, which the driver does once the job is fully printed). Returns
    /// `Err` if the spooler reports a failure state (error, paper out, offline,
    /// deleted) or the wait times out — so the caller can fail the job instead
    /// of reporting a false success.
    fn end_document(&mut self) -> Result<(), String>;

    /// Abort the current document, discarding any spooled data without waiting.
    ///
    /// Used when rendering failed mid-document: there is no point spooling and
    /// waiting on a blank/partial page. On Windows this calls `AbortDoc` so the
    /// spooler job is cancelled rather than printed.
    fn abort_document(&mut self);
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
