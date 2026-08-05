use pdfium_render::prelude::*;
use crate::domain::print_job::PrintJobSettings;
use crate::infrastructure::integrations::pdf_engine::pdfium_loader::load_pdfium;
use crate::infrastructure::platform::printer_api::backend::GraphicsBackend;
use crate::shared::errors::InfrastructureError;
use super::renderer::RenderStrategy;

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
        printer_name: &str,
        settings: &PrintJobSettings,
        backend: &mut dyn GraphicsBackend,
    ) -> Result<(), InfrastructureError> {
        let document = self.pdfium
            .load_pdf_from_file(pdf_path, None)
            .map_err(InfrastructureError::from)?;

        // Rotation from settings (constant for all pages)
        let rotation_angle = {
            let r = settings.rotate.round() as i32;
            ((r % 360) + 360) % 360
        };
        let pdfium_rotation = match rotation_angle {
            90 => PdfPageRenderRotation::Degrees90,
            180 => PdfPageRenderRotation::Degrees180,
            270 => PdfPageRenderRotation::Degrees270,
            _ => PdfPageRenderRotation::None,
        };

        let (paper_w_mm, paper_h_mm) = settings.paper_size.dimensions_mm();

        // Each PDF page is submitted as a separate spooler document.
        //
        // Thermal label printer drivers reset the DC state after EndPage and do
        // not reliably handle multi-page GDI documents: page 1 prints correctly
        // but subsequent pages are misaligned or garbled. One document per label
        // matches how browser-based printing and label utilities work, and is
        // supported by every compliant driver.
        for page in document.pages().iter() {
            backend
                .begin_document(printer_name, "Sapo Print Job", None, paper_w_mm, paper_h_mm)
                .map_err(InfrastructureError::RenderError)?;

            // Read DC metrics after StartDoc so the driver has fully initialised
            // the device context for this label size.
            backend.begin_page();

            let (dpi_x, dpi_y) = backend.get_dpi();
            let (dc_w, dc_h) = backend.get_page_pixels();
            let dpi_x_f = dpi_x as f32;
            let dpi_y_f = dpi_y as f32;

            // If the DC reports valid dimensions, use them to derive the printable
            // area. This is the only reliable way to avoid clipping — paper_size
            // from settings may differ from what the driver actually reports.
            let (printable_w, printable_h, margin_l_px, margin_t_px) = if dc_w > 0 && dc_h > 0 {
                let mm_to_px_x = dpi_x_f / 25.4;
                let mm_to_px_y = dpi_y_f / 25.4;
                let ml = (settings.margin_left as f32 * mm_to_px_x) as i32;
                let mr = (settings.margin_right as f32 * mm_to_px_x) as i32;
                let mt = (settings.margin_top as f32 * mm_to_px_y) as i32;
                let mb = (settings.margin_bottom as f32 * mm_to_px_y) as i32;
                ((dc_w as i32 - ml - mr).max(1), (dc_h as i32 - mt - mb).max(1), ml, mt)
            } else {
                // Fallback: derive from settings paper size (macOS/Linux)
                let pw = (paper_w_mm * dpi_x_f / 25.4
                    - (settings.margin_left + settings.margin_right) as f32 * dpi_x_f / 25.4) as i32;
                let ph = (paper_h_mm * dpi_y_f / 25.4
                    - (settings.margin_top + settings.margin_bottom) as f32 * dpi_y_f / 25.4) as i32;
                let ml = (settings.margin_left as f32 * dpi_x_f / 25.4) as i32;
                let mt = (settings.margin_top as f32 * dpi_y_f / 25.4) as i32;
                (pw.max(1), ph.max(1), ml, mt)
            };

            let width_pts = page.width().value;
            let height_pts = page.height().value;

            // PDF points → DC pixels (1 pt = 1/72 inch)
            let pdf_w_px = width_pts * dpi_x_f / 72.0;
            let pdf_h_px = height_pts * dpi_y_f / 72.0;

            // Scale uniformly to fit inside the printable area
            let scale = (printable_w as f32 / pdf_w_px)
                .min(printable_h as f32 / pdf_h_px);

            let render_w = (pdf_w_px * scale) as i32;
            let render_h = (pdf_h_px * scale) as i32;

            // Centre within printable area
            let pos_x = margin_l_px + (printable_w - render_w) / 2;
            let pos_y = margin_t_px + (printable_h - render_h) / 2;

            // 90°/270° swaps bitmap dimensions
            let (out_w, out_h) = if rotation_angle == 90 || rotation_angle == 270 {
                (render_h, render_w)
            } else {
                (render_w, render_h)
            };

            let render_config = PdfRenderConfig::new()
                .set_fixed_size(out_w, out_h)
                .rotate(pdfium_rotation, true);

            // Render the PDFium bitmap first (no GDI dependency), then blit.
            // PDFium returns BGRx (4 bytes per pixel). Bpp = 32.
            let render_result = page
                .render_with_config(&render_config)
                .map_err(InfrastructureError::from)
                .map(|bitmap| {
                    backend.draw_bitmap(&bitmap.as_raw_bytes(), pos_x, pos_y, out_w as u32, out_h as u32, 32);
                });

            backend.end_page();

            match render_result {
                // Page rendered: close the document and block until the spooler
                // confirms it actually printed. A spooler failure fails the job.
                Ok(()) => backend.end_document().map_err(InfrastructureError::RenderError)?,
                // Render failed: discard the spooler document (no point printing a
                // blank page) and propagate the original error.
                Err(e) => {
                    backend.abort_document();
                    return Err(e);
                }
            }
        }

        Ok(())
    }
}
