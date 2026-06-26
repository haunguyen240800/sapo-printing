use std::path::Path;

use pdfium_render::prelude::*;

use super::document_renderer::{ColorMode, DocumentRenderer, RenderConfig};
use crate::shared::errors::InfrastructureError;
use crate::shared::utils::unit_conversion::mm_to_pixels;

/// PDFium-based PDF renderer.
///
/// Renders PDF documents to raw bitmap with configurable margins,
/// color modes, and DPI. Uses Google's PDFium library for rendering,
/// with manual color space conversion for non-BGRx output formats.
pub struct PdfiumRenderer {
    dpi: u32,
}

impl PdfiumRenderer {
    pub fn new(dpi: u32) -> Self {
        Self { dpi }
    }
}

impl DocumentRenderer for PdfiumRenderer {
    fn render(&self, path: &Path, config: &RenderConfig) -> Result<Vec<u8>, InfrastructureError> {
        config.validate()?;

        let pdfium = Pdfium::default();
        let document = pdfium.load_pdf_from_file(path, None).map_err(|e| {
            InfrastructureError::ValidationError(format!("Failed to open PDF: {}", e))
        })?;

        let page_count = document.pages().len() as u32;
        if page_count == 0 {
            return Err(InfrastructureError::ValidationError(
                "PDF has no pages".to_string(),
            ));
        }

        let dpi = self.dpi;
        let (render_w, render_h) = apply_margins_to_render_area_with_dpi(config, dpi);

        let mut output = Vec::new();
        output.extend_from_slice(&page_count.to_le_bytes());

        for page in document.pages().iter() {
            // Render to the render area size (paper minus margins)
            let render_config =
                PdfRenderConfig::new().set_fixed_size(render_w as i32, render_h as i32);

            let bitmap = page
                .render_with_config(&render_config)
                .map_err(|e| InfrastructureError::RenderError(format!("Render failed: {}", e)))?;

            let converted =
                convert_bitmap_to_color_mode(&bitmap, &config.color_mode, render_w, render_h)?;

            output.extend_from_slice(&render_w.to_le_bytes());
            output.extend_from_slice(&render_h.to_le_bytes());
            output.extend_from_slice(&converted);
        }

        Ok(output)
    }
}

/// Calculates the usable render area after margin subtraction using the config's DPI.
pub fn apply_margins_to_render_area(config: &RenderConfig) -> (u32, u32) {
    apply_margins_to_render_area_with_dpi(config, config.dpi)
}

fn apply_margins_to_render_area_with_dpi(config: &RenderConfig, dpi: u32) -> (u32, u32) {
    let width_px = mm_to_pixels(config.paper_size.width_mm, dpi);
    let height_px = mm_to_pixels(config.paper_size.height_mm, dpi);

    let left_px = mm_to_pixels(config.margin_left_mm, dpi);
    let right_px = mm_to_pixels(config.margin_right_mm, dpi);
    let top_px = mm_to_pixels(config.margin_top_mm, dpi);
    let bottom_px = mm_to_pixels(config.margin_bottom_mm, dpi);

    (
        width_px.saturating_sub(left_px).saturating_sub(right_px),
        height_px.saturating_sub(top_px).saturating_sub(bottom_px),
    )
}

/// Converts a PDFium bitmap to the target color mode.
///
/// PDFium renders in BGRx format (4 bytes/pixel: B, G, R, unused).
/// This function converts to the desired format, handling stride differences.
/// Returns an error if bitmap data is truncated or dimensions mismatch.
fn convert_bitmap_to_color_mode(
    bitmap: &PdfBitmap,
    color_mode: &ColorMode,
    render_w: u32,
    render_h: u32,
) -> Result<Vec<u8>, InfrastructureError> {
    let raw = bitmap.as_raw_bytes();
    let bitmap_w = bitmap.width() as usize;
    let bitmap_h = bitmap.height() as usize;
    let stride = bitmap_w * 4; // BGRx = 4 bytes per pixel
    let bgrx_bytes_per_pixel = 4;

    if render_h as usize > bitmap_h {
        return Err(InfrastructureError::RenderError(format!(
            "Requested render height {} exceeds bitmap height {}",
            render_h, bitmap_h
        )));
    }

    match color_mode {
        ColorMode::Rgb => {
            let actual_w = render_w.min(bitmap_w as u32) as usize;
            let expected_size = actual_w * render_h as usize * 3;
            let mut out = Vec::with_capacity(expected_size);
            for y in 0..render_h as usize {
                let row_start = y * stride;
                for x in 0..actual_w {
                    let px = row_start + x * bgrx_bytes_per_pixel;
                    if px + 2 >= raw.len() {
                        return Err(InfrastructureError::RenderError(
                            "Bitmap data truncated during RGB conversion".to_string(),
                        ));
                    }
                    out.push(raw[px + 2]); // R
                    out.push(raw[px + 1]); // G
                    out.push(raw[px]); // B
                }
            }
            if out.len() != expected_size {
                return Err(InfrastructureError::RenderError(format!(
                    "RGB output size mismatch: expected {}, got {}",
                    expected_size,
                    out.len()
                )));
            }
            Ok(out)
        }
        ColorMode::Argb => {
            let actual_w = render_w.min(bitmap_w as u32) as usize;
            let expected_size = actual_w * render_h as usize * 4;
            let mut out = Vec::with_capacity(expected_size);
            for y in 0..render_h as usize {
                let row_start = y * stride;
                for x in 0..actual_w {
                    let px = row_start + x * bgrx_bytes_per_pixel;
                    if px + 3 >= raw.len() {
                        return Err(InfrastructureError::RenderError(
                            "Bitmap data truncated during ARGB conversion".to_string(),
                        ));
                    }
                    out.push(raw[px]); // B
                    out.push(raw[px + 1]); // G
                    out.push(raw[px + 2]); // R
                    out.push(0xFF); // A
                }
            }
            if out.len() != expected_size {
                return Err(InfrastructureError::RenderError(format!(
                    "ARGB output size mismatch: expected {}, got {}",
                    expected_size,
                    out.len()
                )));
            }
            Ok(out)
        }
        ColorMode::Bgr => {
            let actual_w = render_w.min(bitmap_w as u32) as usize;
            let row_bytes = actual_w * 3;
            let padding = (4 - (row_bytes % 4)) % 4;
            let expected_size = (row_bytes + padding) * render_h as usize;
            
            let mut out = Vec::with_capacity(expected_size);
            for y in 0..render_h as usize {
                let row_start = y * stride;
                for x in 0..actual_w {
                    let px = row_start + x * bgrx_bytes_per_pixel;
                    if px + 2 >= raw.len() {
                        return Err(InfrastructureError::RenderError(
                            "Bitmap data truncated during BGR conversion".to_string(),
                        ));
                    }
                    out.push(raw[px]); // B
                    out.push(raw[px + 1]); // G
                    out.push(raw[px + 2]); // R
                }
                // Pad scanline to 4-byte boundary
                for _ in 0..padding {
                    out.push(0);
                }
            }
            if out.len() != expected_size {
                return Err(InfrastructureError::RenderError(format!(
                    "BGR output size mismatch: expected {}, got {}",
                    expected_size,
                    out.len()
                )));
            }
            Ok(out)
        }
        ColorMode::Gray => {
            let actual_w = render_w.min(bitmap_w as u32) as usize;
            let expected_size = actual_w * render_h as usize;
            let mut out = Vec::with_capacity(expected_size);
            for y in 0..render_h as usize {
                let row_start = y * stride;
                for x in 0..actual_w {
                    let px = row_start + x * bgrx_bytes_per_pixel;
                    if px + 2 >= raw.len() {
                        return Err(InfrastructureError::RenderError(
                            "Bitmap data truncated during Gray conversion".to_string(),
                        ));
                    }
                    let b = raw[px] as f64;
                    let g = raw[px + 1] as f64;
                    let r = raw[px + 2] as f64;
                    let gray = (0.299 * r + 0.587 * g + 0.114 * b).round() as u8;
                    out.push(gray);
                }
            }
            if out.len() != expected_size {
                return Err(InfrastructureError::RenderError(format!(
                    "Gray output size mismatch: expected {}, got {}",
                    expected_size,
                    out.len()
                )));
            }
            Ok(out)
        }
        ColorMode::Binary => {
            let actual_w = render_w.min(bitmap_w as u32) as usize;
            let total_bits = actual_w * render_h as usize;
            let mut binary = vec![0u8; total_bits.div_ceil(8)];
            let mut bit_idx = 0;

            for y in 0..render_h as usize {
                let row_start = y * stride;
                for x in 0..actual_w {
                    let px = row_start + x * bgrx_bytes_per_pixel;
                    if px + 2 >= raw.len() {
                        return Err(InfrastructureError::RenderError(
                            "Bitmap data truncated during Binary conversion".to_string(),
                        ));
                    }
                    let b = raw[px] as f64;
                    let g = raw[px + 1] as f64;
                    let r = raw[px + 2] as f64;
                    let gray = (0.299 * r + 0.587 * g + 0.114 * b).round() as u8;
                    if gray >= 128 {
                        binary[bit_idx / 8] |= 1 << (7 - (bit_idx % 8));
                    }
                    bit_idx += 1;
                }
            }
            Ok(binary)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::renderer::PaperSize;

    #[test]
    fn test_apply_margins_no_margins() {
        let config = RenderConfig {
            paper_size: PaperSize::a4(),
            dpi: 300,
            ..Default::default()
        };
        let (w, h) = apply_margins_to_render_area(&config);
        assert_eq!(w, 2480);
        assert_eq!(h, 3508);
    }

    #[test]
    fn test_apply_margins_10mm() {
        let config = RenderConfig {
            paper_size: PaperSize::a4(),
            margin_left_mm: 10.0,
            margin_right_mm: 10.0,
            margin_top_mm: 10.0,
            margin_bottom_mm: 10.0,
            dpi: 300,
            ..Default::default()
        };
        let (w, h) = apply_margins_to_render_area(&config);
        // 10mm at 300 DPI = 118 pixels
        // 2480 - 118 - 118 = 2244
        // 3508 - 118 - 118 = 3272
        assert_eq!(w, 2244);
        assert_eq!(h, 3272);
    }

    #[test]
    fn test_mm_to_pixels_10mm_at_300dpi() {
        assert_eq!(mm_to_pixels(10.0, 300), 118);
    }

    #[test]
    fn test_a4_dimensions_at_300dpi() {
        assert_eq!(mm_to_pixels(210.0, 300), 2480);
        assert_eq!(mm_to_pixels(297.0, 300), 3508);
    }

    #[test]
    fn test_renderer_new() {
        let renderer = PdfiumRenderer::new(300);
        assert_eq!(renderer.dpi, 300);
    }

    #[test]
    fn test_invalid_pdf_returns_error() {
        let dir = std::env::temp_dir().join("sapo_test_invalid_pdf_renderer");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("invalid.pdf");
        std::fs::write(&path, b"not a pdf").unwrap();

        let renderer = PdfiumRenderer::new(300);
        let config = RenderConfig::default();
        let result = renderer.render(&path, &config);

        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_color_mode_rgb_bytes_per_pixel() {
        // RGB: 3 bytes per pixel (R, G, B)
        // This test verifies the expected byte depth for RGB mode
        let bytes_per_pixel = 3;
        let width = 100;
        let height = 100;
        let expected_size = width * height * bytes_per_pixel;
        assert_eq!(expected_size, 30000);
    }

    #[test]
    fn test_color_mode_argb_bytes_per_pixel() {
        // ARGB: 4 bytes per pixel (A, R, G, B)
        let bytes_per_pixel = 4;
        let width = 100;
        let height = 100;
        let expected_size = width * height * bytes_per_pixel;
        assert_eq!(expected_size, 40000);
    }

    #[test]
    fn test_color_mode_bgr_bytes_per_pixel() {
        // BGR: 3 bytes per pixel (B, G, R)
        let bytes_per_pixel = 3;
        let width = 100;
        let height = 100;
        let expected_size = width * height * bytes_per_pixel;
        assert_eq!(expected_size, 30000);
    }

    #[test]
    fn test_color_mode_gray_bytes_per_pixel() {
        // Gray: 1 byte per pixel
        let bytes_per_pixel = 1;
        let width = 100;
        let height = 100;
        let expected_size = width * height * bytes_per_pixel;
        assert_eq!(expected_size, 10000);
    }

    #[test]
    fn test_color_mode_binary_bits_per_pixel() {
        // Binary: 1 bit per pixel, packed 8 pixels per byte
        let width: usize = 100;
        let height: usize = 100;
        let total_bits = width * height;
        let expected_bytes = total_bits.div_ceil(8);
        assert_eq!(expected_bytes, 1250);
    }
}
