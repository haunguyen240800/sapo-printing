use pdfium_render::prelude::*;
use crate::application::services::layout_engine::LayoutEngine;
use crate::domain::models::PrintJobSettings;
use crate::infrastructure::platform::printer_api::backend::GraphicsBackend;
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
        settings: &PrintJobSettings,
        backend: &mut dyn GraphicsBackend,
    ) {
        let bind = Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./bin/"))
            .or_else(|_| Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./")))
            .or_else(|_| Pdfium::bind_to_system_library());

        let pdfium = match bind {
            Ok(b) => Pdfium::new(b),
            Err(e) => {
                eprintln!("Failed to load PDFium library: {:?}", e);
                return;
            }
        };
        let document = match pdfium.load_pdf_from_file(pdf_path, None) {
            Ok(doc) => doc,
            Err(e) => {
                eprintln!("Failed to load PDF in BitmapRenderStrategy: {}", e);
                return;
            }
        };

        let (dpi_x, dpi_y) = backend.get_dpi();
        
        for page in document.pages().iter() {
            backend.begin_page();

            let width_points = page.width().value;
            let height_points = page.height().value;

            // Convert points (1/72 inch) to pixels using target DPI
            let transform = LayoutEngine::calculate(settings, width_points, height_points);
            let dpi_x_f = dpi_x as f32;
            let dpi_y_f = dpi_y as f32;
            let scale_x = transform.scale_x;
            let scale_y = transform.scale_y;
            
            let render_w = (width_points * scale_x * dpi_x_f / 72.0) as i32;
            let render_h = (height_points * scale_y * dpi_y_f / 72.0) as i32;
            
            let pos_x = (transform.translate_x * dpi_x_f / 72.0) as i32;
            let pos_y = (transform.translate_y * dpi_y_f / 72.0) as i32;

            let mut render_config = PdfRenderConfig::new().set_fixed_size(render_w, render_h);


            if let Ok(bitmap) = page.render_with_config(&render_config) {
                let raw = bitmap.as_raw_bytes();
                // PDFium returns BGRx (4 bytes per pixel). Bpp = 32.
                backend.draw_bitmap(&raw, pos_x, pos_y, render_w as u32, render_h as u32, 32);
            }

            backend.end_page();
        }
    }
}
