use crate::domain::print_job::services::layout_engine::LayoutEngine;
use crate::domain::print_job::PrintJobSettings;
use crate::infrastructure::integrations::pdf_engine::pdfium_loader::load_pdfium;
use crate::infrastructure::platform::printer_api::backend::{GraphicsBackend, NativeGraphicsContext};
use crate::shared::errors::InfrastructureError;
use super::renderer::RenderStrategy;

/// Placeholder strategy that walks the PDF using native graphics contexts.
///
/// NOTE: the FFI bridge to `FPDF_RenderPage` is not implemented yet on any
/// platform. Calling this returns `Err(InfrastructureError::RenderError)` so
/// jobs are not silently marked completed when the native path is selected.
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
        _printer_name: &str,
        settings: &PrintJobSettings,
        backend: &mut dyn GraphicsBackend,
    ) -> Result<(), InfrastructureError> {
        let pdfium = load_pdfium()?;
        let document = pdfium
            .load_pdf_from_file(pdf_path, None)
            .map_err(InfrastructureError::from)?;

        let native_ctx = backend.native_context();

        for page in document.pages().iter() {
            backend.begin_page();

            let width_points = page.width().value;
            let height_points = page.height().value;

            let transform = LayoutEngine::calculate(settings, width_points, height_points);
            let dpi_f = settings.dpi as f32;
            let render_w = (width_points * transform.scale_x * dpi_f / 72.0) as i32;
            let render_h = (height_points * transform.scale_y * dpi_f / 72.0) as i32;

            // FFI bridge to FPDF_RenderPage is not wired yet. Surface this explicitly
            // so the worker fails the job rather than completing a blank page.
            let context_desc = match native_ctx {
                NativeGraphicsContext::Windows(hdc) => format!("Windows HDC={} size={}x{}", hdc, render_w, render_h),
                NativeGraphicsContext::Mac(cg_ctx) => format!("Mac CGContext={} size={}x{}", cg_ctx, render_w, render_h),
                NativeGraphicsContext::Linux(cairo_ctx) => format!("Cairo={} size={}x{}", cairo_ctx, render_w, render_h),
            };

            backend.end_page();
            return Err(InfrastructureError::RenderError(format!(
                "NativePdfRenderStrategy not implemented ({})",
                context_desc
            )));
        }

        Ok(())
    }
}
