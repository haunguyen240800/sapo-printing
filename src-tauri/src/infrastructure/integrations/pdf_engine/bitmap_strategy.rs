use pdfium_render::prelude::*;
use crate::domain::print_job::services::layout_engine::LayoutEngine;
use crate::domain::print_job::PrintJobSettings;
use crate::infrastructure::integrations::pdf_engine::pdfium_loader::load_pdfium;
use crate::infrastructure::platform::printer_api::backend::GraphicsBackend;
use crate::shared::errors::InfrastructureError;
use super::renderer::RenderStrategy;

/// Holds the PDFium instance for the lifetime of the application.
///
/// PDFium's library bindings are a process-wide singleton — calling
/// `bind_to_library` more than once returns `AlreadyInitialized`. Storing
/// `Pdfium` here ensures it is initialised exactly once (when this struct is
/// constructed) and never dropped while jobs are still being processed.
pub struct BitmapRenderStrategy {
    pdfium: Pdfium,
}

impl BitmapRenderStrategy {
    pub fn new() -> Result<Self, InfrastructureError> {
        Ok(Self { pdfium: load_pdfium()? })
    }
}

impl RenderStrategy for BitmapRenderStrategy {
    fn render(
        &self,
        pdf_path: &str,
        settings: &PrintJobSettings,
        backend: &mut dyn GraphicsBackend,
    ) -> Result<(), InfrastructureError> {
        let document = self.pdfium
            .load_pdf_from_file(pdf_path, None)
            .map_err(InfrastructureError::from)?;

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

            let rotation_angle = {
                let r = transform.rotation.round() as i32;
                ((r % 360) + 360) % 360
            };
            let pdfium_rotation = match rotation_angle {
                90 => PdfPageRenderRotation::Degrees90,
                180 => PdfPageRenderRotation::Degrees180,
                270 => PdfPageRenderRotation::Degrees270,
                _ => PdfPageRenderRotation::None,
            };
            // 90°/270° swaps width and height in the output bitmap.
            let (out_w, out_h) = if rotation_angle == 90 || rotation_angle == 270 {
                (render_h, render_w)
            } else {
                (render_w, render_h)
            };

            let render_config = PdfRenderConfig::new()
                .set_fixed_size(out_w, out_h)
                .rotate(pdfium_rotation, true);

            let bitmap = page
                .render_with_config(&render_config)
                .map_err(InfrastructureError::from)?;
            let raw = bitmap.as_raw_bytes();
            // PDFium returns BGRx (4 bytes per pixel). Bpp = 32.
            backend.draw_bitmap(&raw, pos_x, pos_y, out_w as u32, out_h as u32, 32);

            backend.end_page();
        }

        Ok(())
    }
}
