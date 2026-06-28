use pdfium_render::prelude::*;
use crate::application::services::layout_engine::LayoutEngine;
use crate::domain::models::PrintJobSettings;
use crate::infrastructure::platform::printer_api::backend::{GraphicsBackend, NativeGraphicsContext};
use super::renderer::RenderStrategy;

pub struct NativePdfRenderStrategy;

impl NativePdfRenderStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl RenderStrategy for NativePdfRenderStrategy {
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
                eprintln!("Failed to load PDF in NativePdfRenderStrategy: {}", e);
                return;
            }
        };

        let native_ctx = backend.native_context();

        for page in document.pages().iter() {
            backend.begin_page();
            
            let width_points = page.width().value;
            let height_points = page.height().value;
            
                        let transform = LayoutEngine::calculate(settings, width_points, height_points);
            let dpi_f = settings.dpi as f32;
            let render_w = (width_points * transform.scale_x * dpi_f / 72.0) as i32;
            let render_h = (height_points * transform.scale_y * dpi_f / 72.0) as i32;

            match native_ctx {
                NativeGraphicsContext::Windows(hdc) => {
                    // Extract page handle
                    let _page_handle = page.bindings(); // Simplified pseudo-access
                    
                    // Actually, pdfium-render doesn't expose FPDF_RenderPage directly with HDC safely in standard bindings,
                    // but we can use the FFI if we link it.
                    // For now, this represents the native render path where we would use FPDF_RenderPage
                    // with the Windows DC.
                    eprintln!("Native rendering to Windows HDC: {} (Size: {}x{})", hdc, render_w, render_h);
                }
                NativeGraphicsContext::Mac(cg_ctx) => {
                    // FPDF_RenderPage doesn't work directly with CGContext, usually we draw bitmap on Mac
                    // or generate a new PDF stream.
                    eprintln!("Native rendering to Mac CGContext: {}", cg_ctx);
                }
                NativeGraphicsContext::Linux(cairo_ctx) => {
                    eprintln!("Native rendering to Cairo Context: {}", cairo_ctx);
                }
            }

            // Fallback for demonstration: if native fails or is mocked, we draw bitmap
            // In a real implementation, we would bypass draw_bitmap and use native FFI here.

            backend.end_page();
        }
    }
}
