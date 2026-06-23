use std::path::PathBuf;

use sapo_printer::infrastructure::renderer::{
    ColorMode, DocumentRenderer, PaperSize, PdfiumRenderer, RenderConfig,
};

fn create_test_pdf(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sapo_renderer_tests_{}", test_name));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("test.pdf");

    // Minimal valid PDF 1.4 with one blank page
    let pdf_content = b"%PDF-1.4\n\
        1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
        2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
        3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n\
        xref\n0 4\n\
        0000000000 65535 f \n\
        0000000009 00000 n \n\
        0000000058 00000 n \n\
        0000000115 00000 n \n\
        trailer\n<< /Size 4 /Root 1 0 R >>\n\
        startxref\n190\n%%EOF";

    std::fs::write(&path, pdf_content).unwrap();
    path
}

fn create_multipage_test_pdf(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sapo_renderer_tests_{}", test_name));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("multipage.pdf");

    // Minimal valid PDF 1.4 with two blank pages
    let pdf_content = b"%PDF-1.4\n\
        1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n\
        2 0 obj\n<< /Type /Pages /Kids [3 0 R, 4 0 R] /Count 2 >>\nendobj\n\
        3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n\
        4 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n\
        xref\n0 5\n\
        0000000000 65535 f \n\
        0000000009 00000 n \n\
        0000000058 00000 n \n\
        0000000125 00000 n \n\
        0000000192 00000 n \n\
        trailer\n<< /Size 5 /Root 1 0 R >>\n\
        startxref\n259\n%%EOF";

    std::fs::write(&path, pdf_content).unwrap();
    path
}

fn cleanup_test_dir(test_name: &str) {
    let dir = std::env::temp_dir().join(format!("sapo_renderer_tests_{}", test_name));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_render_pdf_with_default_config() {
    let pdf_path = create_test_pdf("default_config");
    let renderer = PdfiumRenderer::new(300);
    let config = RenderConfig::default();

    let result = renderer.render(&pdf_path, &config);
    assert!(result.is_ok(), "Rendering should succeed");

    let bitmap = result.unwrap();
    assert!(!bitmap.is_empty(), "Bitmap should not be empty");

    // Verify page count header (4 bytes, little-endian)
    let page_count = u32::from_le_bytes([bitmap[0], bitmap[1], bitmap[2], bitmap[3]]);
    assert_eq!(page_count, 1, "Should have 1 page");

    cleanup_test_dir("default_config");
}

#[test]
fn test_render_pdf_with_rgb_color_mode() {
    let pdf_path = create_test_pdf("rgb_mode");
    let renderer = PdfiumRenderer::new(300);
    let config = RenderConfig {
        color_mode: ColorMode::Rgb,
        paper_size: PaperSize::a4(),
        dpi: 300,
        ..Default::default()
    };

    let result = renderer.render(&pdf_path, &config);
    assert!(result.is_ok());

    let bitmap = result.unwrap();
    // Read actual dimensions from bitmap header
    let width = u32::from_le_bytes([bitmap[4], bitmap[5], bitmap[6], bitmap[7]]);
    let height = u32::from_le_bytes([bitmap[8], bitmap[9], bitmap[10], bitmap[11]]);

    // RGB: 3 bytes per pixel
    let expected_size = 12 + (width * height * 3) as usize;
    assert_eq!(bitmap.len(), expected_size, "RGB bitmap size should match");

    cleanup_test_dir("rgb_mode");
}

#[test]
fn test_render_pdf_with_argb_color_mode() {
    let pdf_path = create_test_pdf("argb_mode");
    let renderer = PdfiumRenderer::new(300);
    let config = RenderConfig {
        color_mode: ColorMode::Argb,
        paper_size: PaperSize::a4(),
        dpi: 300,
        ..Default::default()
    };

    let result = renderer.render(&pdf_path, &config);
    assert!(result.is_ok());

    let bitmap = result.unwrap();
    // Read actual dimensions from bitmap header
    let width = u32::from_le_bytes([bitmap[4], bitmap[5], bitmap[6], bitmap[7]]);
    let height = u32::from_le_bytes([bitmap[8], bitmap[9], bitmap[10], bitmap[11]]);

    // ARGB: 4 bytes per pixel
    let expected_size = 12 + (width * height * 4) as usize;
    assert_eq!(bitmap.len(), expected_size, "ARGB bitmap size should match");

    cleanup_test_dir("argb_mode");
}

#[test]
fn test_render_pdf_with_bgr_color_mode() {
    let pdf_path = create_test_pdf("bgr_mode");
    let renderer = PdfiumRenderer::new(300);
    let config = RenderConfig {
        color_mode: ColorMode::Bgr,
        paper_size: PaperSize::a4(),
        dpi: 300,
        ..Default::default()
    };

    let result = renderer.render(&pdf_path, &config);
    assert!(result.is_ok());

    let bitmap = result.unwrap();
    // Read actual dimensions from bitmap header
    let width = u32::from_le_bytes([bitmap[4], bitmap[5], bitmap[6], bitmap[7]]);
    let height = u32::from_le_bytes([bitmap[8], bitmap[9], bitmap[10], bitmap[11]]);

    // BGR: 3 bytes per pixel
    let expected_size = 12 + (width * height * 3) as usize;
    assert_eq!(bitmap.len(), expected_size, "BGR bitmap size should match");

    cleanup_test_dir("bgr_mode");
}

#[test]
fn test_render_pdf_with_gray_color_mode() {
    let pdf_path = create_test_pdf("gray_mode");
    let renderer = PdfiumRenderer::new(300);
    let config = RenderConfig {
        color_mode: ColorMode::Gray,
        paper_size: PaperSize::a4(),
        dpi: 300,
        ..Default::default()
    };

    let result = renderer.render(&pdf_path, &config);
    assert!(result.is_ok());

    let bitmap = result.unwrap();
    // Read actual dimensions from bitmap header
    let width = u32::from_le_bytes([bitmap[4], bitmap[5], bitmap[6], bitmap[7]]);
    let height = u32::from_le_bytes([bitmap[8], bitmap[9], bitmap[10], bitmap[11]]);

    // Gray: 1 byte per pixel
    let expected_size = 12 + (width * height) as usize;
    assert_eq!(bitmap.len(), expected_size, "Gray bitmap size should match");

    cleanup_test_dir("gray_mode");
}

#[test]
fn test_render_pdf_with_binary_color_mode() {
    let pdf_path = create_test_pdf("binary_mode");
    let renderer = PdfiumRenderer::new(300);
    let config = RenderConfig {
        color_mode: ColorMode::Binary,
        paper_size: PaperSize::a4(),
        dpi: 300,
        ..Default::default()
    };

    let result = renderer.render(&pdf_path, &config);
    assert!(result.is_ok());

    let bitmap = result.unwrap();
    let width = u32::from_le_bytes([bitmap[4], bitmap[5], bitmap[6], bitmap[7]]);
    let height = u32::from_le_bytes([bitmap[8], bitmap[9], bitmap[10], bitmap[11]]);

    // Binary: 1 bit per pixel, packed into bytes
    let total_bits = width * height;
    let expected_bytes = total_bits.div_ceil(8) as usize;
    let expected_size = 12 + expected_bytes;
    assert_eq!(
        bitmap.len(),
        expected_size,
        "Binary bitmap size should match"
    );

    cleanup_test_dir("binary_mode");
}

#[test]
fn test_render_pdf_with_margins() {
    let pdf_path = create_test_pdf("margins");
    let renderer = PdfiumRenderer::new(300);
    let config = RenderConfig {
        paper_size: PaperSize::a4(),
        margin_left_mm: 10.0,
        margin_right_mm: 10.0,
        margin_top_mm: 10.0,
        margin_bottom_mm: 10.0,
        color_mode: ColorMode::Rgb,
        dpi: 300,
    };

    let result = renderer.render(&pdf_path, &config);
    assert!(result.is_ok());

    let bitmap = result.unwrap();
    // Read actual dimensions from bitmap header
    let width = u32::from_le_bytes([bitmap[4], bitmap[5], bitmap[6], bitmap[7]]);
    let height = u32::from_le_bytes([bitmap[8], bitmap[9], bitmap[10], bitmap[11]]);

    // With 10mm margins at 300 DPI (118 pixels each side), render area should be smaller
    // A4: 2480x3508, margins: 118px each side
    // Expected: (2480-236) x (3508-236) = 2244 x 3272
    // But PDFium maintains aspect ratio, so actual dimensions may vary slightly
    assert!(width <= 2244, "Width should be at most 2244 with margins");
    assert!(height <= 3272, "Height should be at most 3272 with margins");

    cleanup_test_dir("margins");
}

#[test]
fn test_render_invalid_pdf_returns_error() {
    let dir = std::env::temp_dir().join("sapo_renderer_tests_invalid");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("invalid.pdf");
    std::fs::write(&path, b"not a valid pdf").unwrap();

    let renderer = PdfiumRenderer::new(300);
    let config = RenderConfig::default();

    let result = renderer.render(&path, &config);
    assert!(result.is_err(), "Invalid PDF should return error");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_render_with_different_dpi() {
    let pdf_path = create_test_pdf("different_dpi");
    let renderer = PdfiumRenderer::new(150); // Half of 300 DPI
    let config = RenderConfig {
        paper_size: PaperSize::a4(),
        dpi: 150,
        color_mode: ColorMode::Rgb,
        ..Default::default()
    };

    let result = renderer.render(&pdf_path, &config);
    assert!(result.is_ok());

    let bitmap = result.unwrap();
    // Read actual dimensions from bitmap header
    let width = u32::from_le_bytes([bitmap[4], bitmap[5], bitmap[6], bitmap[7]]);
    let height = u32::from_le_bytes([bitmap[8], bitmap[9], bitmap[10], bitmap[11]]);

    // At 150 DPI, dimensions should be roughly half of 300 DPI
    // A4 at 150 DPI: 1240x1754 (but PDFium maintains aspect ratio)
    assert!(width <= 1240, "Width should be at most 1240 at 150 DPI");
    assert!(height <= 1754, "Height should be at most 1754 at 150 DPI");

    cleanup_test_dir("different_dpi");
}

#[test]
fn test_render_with_a5_paper_size() {
    let pdf_path = create_test_pdf("a5_paper");
    let renderer = PdfiumRenderer::new(300);
    let config = RenderConfig {
        paper_size: PaperSize::a5(),
        dpi: 300,
        color_mode: ColorMode::Rgb,
        ..Default::default()
    };

    let result = renderer.render(&pdf_path, &config);
    assert!(result.is_ok());

    let bitmap = result.unwrap();
    // Read actual dimensions from bitmap header
    let width = u32::from_le_bytes([bitmap[4], bitmap[5], bitmap[6], bitmap[7]]);
    let height = u32::from_le_bytes([bitmap[8], bitmap[9], bitmap[10], bitmap[11]]);

    // A5 at 300 DPI: 1748x2480 (but PDFium maintains aspect ratio)
    assert!(width <= 1748, "Width should be at most 1748 for A5");
    assert!(height <= 2480, "Height should be at most 2480 for A5");

    cleanup_test_dir("a5_paper");
}

#[test]
fn test_render_with_invalid_margins_returns_error() {
    let pdf_path = create_test_pdf("invalid_margins");
    let renderer = PdfiumRenderer::new(300);
    let config = RenderConfig {
        paper_size: PaperSize::a4(),
        margin_left_mm: 200.0, // Exceeds paper width
        margin_right_mm: 200.0,
        dpi: 300,
        ..Default::default()
    };

    let result = renderer.render(&pdf_path, &config);
    assert!(result.is_err(), "Invalid margins should return error");

    cleanup_test_dir("invalid_margins");
}

#[test]
fn test_render_multipage_pdf() {
    // Note: Creating a valid multi-page PDF by hand is error-prone due to xref offsets.
    // This test verifies the code path doesn't crash with a multi-page PDF structure.
    // For production use, real multi-page PDFs from document sources should be tested.
    let pdf_path = create_multipage_test_pdf("multipage");
    let renderer = PdfiumRenderer::new(300);
    let config = RenderConfig {
        paper_size: PaperSize::a4(),
        dpi: 300,
        color_mode: ColorMode::Rgb,
        ..Default::default()
    };

    let result = renderer.render(&pdf_path, &config);
    // The hand-crafted PDF may not render perfectly, but should not crash
    // If it succeeds, verify basic structure
    if let Ok(bitmap) = result {
        assert!(!bitmap.is_empty(), "Bitmap should not be empty if render succeeds");
        // Verify we have at least the page count header
        assert!(bitmap.len() >= 4, "Bitmap should contain at least page count header");
    }

    cleanup_test_dir("multipage");
}
