use crate::domain::print_job::value_objects::{PrintJobSettings, Transform};

pub struct LayoutEngine;

impl LayoutEngine {
    /// Calculates the transformation required to fit a PDF page onto the physical paper,
    /// accounting for paper size, margins, and orientation.
    ///
    /// Inputs:
    /// - `pdf_width`, `pdf_height`: Original PDF page dimensions in points (1/72 inch).
    /// Output:
    /// - `Transform`: scale and translation (in points) to apply before rendering.
    pub fn calculate(settings: &PrintJobSettings, pdf_width: f32, pdf_height: f32) -> Transform {
        // 1. Determine paper size in mm (PaperSize VO owns the lookup table)
        let (mut paper_w_mm, mut paper_h_mm) = settings.paper_size.dimensions_mm();

        // Apply orientation flip if landscape
        if settings.orientation.eq_ignore_ascii_case("LANDSCAPE") {
            std::mem::swap(&mut paper_w_mm, &mut paper_h_mm);
        }

        // Conversion factor: mm to points (1 inch = 25.4 mm = 72 points)
        let mm_to_pts = 72.0 / 25.4;

        // 2. Convert to points
        let paper_w_pts = paper_w_mm * mm_to_pts;
        let paper_h_pts = paper_h_mm * mm_to_pts;

        let margin_l_pts = settings.margin_left as f32 * mm_to_pts;
        let margin_r_pts = settings.margin_right as f32 * mm_to_pts;
        let margin_t_pts = settings.margin_top as f32 * mm_to_pts;
        let margin_b_pts = settings.margin_bottom as f32 * mm_to_pts;

        // 3. Printable area
        let printable_w = paper_w_pts - margin_l_pts - margin_r_pts;
        let printable_h = paper_h_pts - margin_t_pts - margin_b_pts;

        if printable_w <= 0.0 || printable_h <= 0.0 {
            // Invalid margins (larger than paper), fallback to 1.0 scale
            return Transform {
                scale_x: 1.0,
                scale_y: 1.0,
                translate_x: 0.0,
                translate_y: 0.0,
                rotation: settings.rotate,
            };
        }

        // 4. Calculate scale (fit proportionally)
        let scale_x = printable_w / pdf_width;
        let scale_y = printable_h / pdf_height;
        let scale = scale_x.min(scale_y);

        // 5. Calculate translation (center in printable area)
        let scaled_pdf_w = pdf_width * scale;
        let scaled_pdf_h = pdf_height * scale;

        let translate_x = margin_l_pts + (printable_w - scaled_pdf_w) / 2.0;
        let translate_y = margin_t_pts + (printable_h - scaled_pdf_h) / 2.0;

        Transform {
            scale_x: scale,
            scale_y: scale,
            translate_x,
            translate_y,
            rotation: settings.rotate,
        }
    }
}
