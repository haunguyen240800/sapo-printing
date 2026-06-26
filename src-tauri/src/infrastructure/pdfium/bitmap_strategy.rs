use pdfium_render::prelude::*;
use crate::domain::layout::Transform;
use crate::domain::settings::PrintSettings;
use crate::infrastructure::graphics::backend::GraphicsBackend;
use super::renderer::RenderStrategy;

pub struct BitmapRenderStrategy;

impl BitmapRenderStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl RenderStrategy for BitmapRenderStrategy {
    fn render(
        &self,
        pdf_path: &str,
        _transform: &Transform,
        settings: &PrintSettings,
        backend: &mut dyn GraphicsBackend,
    ) {
        let pdfium = Pdfium::default();
        let document = match pdfium.load_pdf_from_file(pdf_path, None) {
            Ok(doc) => doc,
            Err(e) => {
                eprintln!("Failed to load PDF in BitmapRenderStrategy: {}", e);
                return;
            }
        };

        let dpi = settings.dpi;

        for page in document.pages().iter() {
            backend.begin_page();

            let width_points = page.width().value;
            let height_points = page.height().value;

            // Convert points (1/72 inch) to pixels using target DPI
            let render_w = (width_points * dpi as f32 / 72.0) as i32;
            let render_h = (height_points * dpi as f32 / 72.0) as i32;
            
            // Note: Transform applies scaling/offset, simplified here
            let render_config = PdfRenderConfig::new().set_fixed_size(render_w, render_h);

            if let Ok(bitmap) = page.render_with_config(&render_config) {
                let raw = bitmap.as_raw_bytes();
                // PDFium returns BGRx (4 bytes per pixel). Bpp = 32.
                backend.draw_bitmap(&raw, render_w as u32, render_h as u32, 32);
            }

            backend.end_page();
        }
    }
}
