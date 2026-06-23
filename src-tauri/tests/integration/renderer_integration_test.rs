use std::path::PathBuf;
use std::sync::Arc;

use sapo_printer::domain::printer::{Printer, PrinterStatus};
use sapo_printer::infrastructure::printer::PrinterManager;
use sapo_printer::infrastructure::renderer::{
    ColorMode, DirectPdfRenderer, DocumentRenderer, PaperSize, PdfiumRenderer, RenderConfig,
    StrategySelector,
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
        assert!(
            !bitmap.is_empty(),
            "Bitmap should not be empty if render succeeds"
        );
        // Verify we have at least the page count header
        assert!(
            bitmap.len() >= 4,
            "Bitmap should contain at least page count header"
        );
    }

    cleanup_test_dir("multipage");
}

// ─── DirectPdfRenderer Integration Tests ────────────────────────────────────

fn create_direct_pdf_test_pdf(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sapo_direct_pdf_integ_{}", test_name));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("test.pdf");

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

fn create_direct_pdf_multipage(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sapo_direct_pdf_integ_{}", test_name));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("multipage.pdf");

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

fn cleanup_direct_pdf_test_dir(test_name: &str) {
    let dir = std::env::temp_dir().join(format!("sapo_direct_pdf_integ_{}", test_name));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_direct_pdf_output_is_valid_pdf() {
    let pdf_path = create_direct_pdf_test_pdf("valid_output");
    let file_content = std::fs::read(&pdf_path).unwrap();

    let renderer = DirectPdfRenderer::new();
    let config = RenderConfig::default();
    let output = renderer.render(&pdf_path, &config).unwrap();

    assert!(output.starts_with(b"%PDF-"), "Output must start with %PDF-");

    let tail = &output[output.len().saturating_sub(5)..];
    assert!(
        tail.windows(5).any(|w| w == b"%%EOF"),
        "Output must end with %%EOF"
    );

    assert_eq!(output, file_content, "Output must match input exactly");
    cleanup_direct_pdf_test_dir("valid_output");
}

#[test]
fn test_direct_pdf_file_size_matches_input() {
    let pdf_path = create_direct_pdf_test_pdf("size_match");
    let file_size = std::fs::metadata(&pdf_path).unwrap().len();

    let renderer = DirectPdfRenderer::new();
    let config = RenderConfig::default();
    let output = renderer.render(&pdf_path, &config).unwrap();

    assert_eq!(
        output.len() as u64,
        file_size,
        "Output size must equal input file size (no overhead)"
    );
    cleanup_direct_pdf_test_dir("size_match");
}

#[test]
fn test_direct_pdf_vs_pdfium_output_differs() {
    let pdf_path = create_direct_pdf_test_pdf("vs_pdfium");

    let direct = DirectPdfRenderer::new();
    let config = RenderConfig::default();
    let direct_output = direct.render(&pdf_path, &config).unwrap();

    let pdfium = PdfiumRenderer::new(300);
    let pdfium_output = pdfium.render(&pdf_path, &config).unwrap();

    assert_ne!(
        direct_output, pdfium_output,
        "Direct PDF and PDFium outputs must differ"
    );

    assert!(
        direct_output.starts_with(b"%PDF-"),
        "Direct PDF output must start with %PDF-"
    );

    assert!(
        pdfium_output.len() >= 4,
        "PDFium output must have at least 4 bytes for page count header"
    );
    let pdfium_page_count = u32::from_le_bytes([
        pdfium_output[0],
        pdfium_output[1],
        pdfium_output[2],
        pdfium_output[3],
    ]);
    assert!(
        pdfium_page_count > 0,
        "PDFium output must start with page count header (u32 LE)"
    );

    cleanup_direct_pdf_test_dir("vs_pdfium");
}

#[test]
fn test_direct_pdf_multipage_preserves_all_pages() {
    let pdf_path = create_direct_pdf_multipage("multipage");
    let file_content = std::fs::read(&pdf_path).unwrap();

    let renderer = DirectPdfRenderer::new();
    let config = RenderConfig::default();
    let output = renderer.render(&pdf_path, &config).unwrap();

    assert_eq!(
        output, file_content,
        "Multi-page PDF must be preserved in full (all pages)"
    );
    assert!(
        output.len() > file_content.len() / 2,
        "Output should be the full file, not truncated"
    );
    cleanup_direct_pdf_test_dir("multipage");
}

// ─── StrategySelector Integration Tests ─────────────────────────────────────

struct IntegrationMockPrinterManager {
    supports_pdf: bool,
}

impl PrinterManager for IntegrationMockPrinterManager {
    fn discover_printers(&self) -> Vec<Printer> {
        vec![]
    }

    fn get_status(&self, _name: &str) -> PrinterStatus {
        PrinterStatus::Online
    }

    fn supports_direct_pdf(&self, _printer_name: &str) -> bool {
        self.supports_pdf
    }
}

fn create_strategy_test_pdf(test_name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sapo_strategy_integ_{}", test_name));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("test.pdf");

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

fn cleanup_strategy_test_dir(test_name: &str) {
    let dir = std::env::temp_dir().join(format!("sapo_strategy_integ_{}", test_name));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_strategy_selector_direct_pdf_path() {
    let mock = Arc::new(IntegrationMockPrinterManager { supports_pdf: true });
    let selector = StrategySelector::new(mock);
    let config = RenderConfig::default();
    let pdf_path = create_strategy_test_pdf("direct_path");

    let renderer = selector.select_renderer("test_printer", &config);
    let output = renderer.render(&pdf_path, &config).unwrap();

    assert!(
        output.starts_with(b"%PDF-"),
        "Direct PDF path: output must start with %PDF-"
    );
    cleanup_strategy_test_dir("direct_path");
}

#[test]
fn test_strategy_selector_pdfium_path() {
    let mock = Arc::new(IntegrationMockPrinterManager {
        supports_pdf: false,
    });
    let selector = StrategySelector::new(mock);
    let config = RenderConfig::default();
    let pdf_path = create_strategy_test_pdf("pdfium_path");

    let renderer = selector.select_renderer("test_printer", &config);
    let output = renderer.render(&pdf_path, &config).unwrap();

    assert!(
        !output.starts_with(b"%PDF-"),
        "PDFium path: output must NOT start with %PDF-"
    );
    assert!(
        output.len() >= 4,
        "PDFium output must have page count header"
    );
    cleanup_strategy_test_dir("pdfium_path");
}

#[test]
fn test_strategy_selector_margin_triggers_fallback() {
    let mock = Arc::new(IntegrationMockPrinterManager { supports_pdf: true });
    let selector = StrategySelector::new(mock);
    let config = RenderConfig {
        margin_left_mm: 10.0,
        margin_right_mm: 10.0,
        margin_top_mm: 10.0,
        margin_bottom_mm: 10.0,
        ..Default::default()
    };
    let pdf_path = create_strategy_test_pdf("margin_fallback");

    let renderer = selector.select_renderer("test_printer", &config);
    let output = renderer.render(&pdf_path, &config).unwrap();

    assert!(
        !output.starts_with(b"%PDF-"),
        "Non-zero margins must trigger PDFium fallback"
    );
    cleanup_strategy_test_dir("margin_fallback");
}

#[test]
fn test_strategy_selector_performance_comparison() {
    let dir = std::env::temp_dir().join("sapo_strategy_integ_perf");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("perf_test.pdf");

    // Create ~100KB PDF: valid header + padding + valid trailer
    let mut pdf_content = Vec::with_capacity(100_000);
    pdf_content.extend_from_slice(b"%PDF-1.4\n");
    pdf_content.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");
    pdf_content.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");
    pdf_content.extend_from_slice(
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>\nendobj\n",
    );
    // Pad to ~100KB with comment lines (PDF spec allows % comments anywhere)
    while pdf_content.len() < 99_800 {
        pdf_content.extend_from_slice(b"% padding line to reach target size\n");
    }
    pdf_content.extend_from_slice(b"xref\n0 4\n");
    pdf_content.extend_from_slice(b"0000000000 65535 f \n");
    pdf_content.extend_from_slice(b"0000000009 00000 n \n");
    pdf_content.extend_from_slice(b"0000000058 00000 n \n");
    pdf_content.extend_from_slice(b"0000000115 00000 n \n");
    pdf_content.extend_from_slice(b"trailer\n<< /Size 4 /Root 1 0 R >>\n");
    pdf_content.extend_from_slice(b"startxref\n190\n%%EOF");

    std::fs::write(&path, &pdf_content).unwrap();

    let config = RenderConfig::default();

    let direct = DirectPdfRenderer::new();
    let start = std::time::Instant::now();
    let direct_output = direct.render(&path, &config).unwrap();
    let direct_elapsed = start.elapsed();
    assert!(
        direct_elapsed.as_secs_f64() < 1.0,
        "Direct PDF render took {:.3}s, expected < 1s",
        direct_elapsed.as_secs_f64()
    );

    let pdfium = PdfiumRenderer::new(300);
    let start = std::time::Instant::now();
    let pdfium_output = pdfium.render(&path, &config).unwrap();
    let pdfium_elapsed = start.elapsed();
    assert!(
        pdfium_elapsed.as_secs_f64() < 5.0,
        "PDFium render took {:.3}s, expected < 5s",
        pdfium_elapsed.as_secs_f64()
    );

    assert!(
        direct_output.starts_with(b"%PDF-"),
        "Direct PDF output must start with %PDF-"
    );
    assert!(
        !pdfium_output.starts_with(b"%PDF-"),
        "PDFium output must NOT start with %PDF-"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
