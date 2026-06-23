use std::io::Read;
use std::path::Path;

use super::document_renderer::{DocumentRenderer, RenderConfig};
use crate::shared::errors::InfrastructureError;

/// Fast-path PDF renderer — sends raw PDF bytes directly to the printer.
///
/// # Purpose
///
/// When the target printer supports native PDF rendering, this strategy
/// bypasses all bitmap conversion and sends the PDF file contents unchanged.
/// The printer's own rasterizer handles margins, color, and page layout.
///
/// # When to use
///
/// Selected by `StrategySelector` (Story 3.4) when printer capability
/// detection confirms native PDF support. Ideal for high-throughput
/// environments (100+ invoices/minute).
///
/// # Trade-offs
///
/// | Aspect | DirectPdfRenderer | PdfiumRenderer |
/// |--------|-------------------|----------------|
/// | Speed | ~0.5s per job | ~2-3s per job |
/// | Margin control | None (printer handles) | Full (software-applied) |
/// | Color mode | None (printer handles) | RGB/ARGB/BGR/Gray/Binary |
/// | CPU usage | Minimal (file read only) | Moderate (rendering) |
/// | Output format | Raw PDF bytes | Bitmap `[page_count][w][h][pixels]` |
///
/// # Output format
///
/// Returns the exact bytes of the input PDF file — no transformation,
/// no rendering, no margin application, no color conversion.
///
/// # Integration
///
/// - Selected by `StrategySelector` (Story 3.4)
/// - Used by `QueueWorker` (Story 3.5) via `Arc<dyn DocumentRenderer>`
pub struct DirectPdfRenderer;

impl DirectPdfRenderer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DirectPdfRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl DocumentRenderer for DirectPdfRenderer {
    fn render(&self, path: &Path, _config: &RenderConfig) -> Result<Vec<u8>, InfrastructureError> {
        validate_pdf_header(path)?;
        std::fs::read(path).map_err(InfrastructureError::from)
    }
}

fn validate_pdf_header(path: &Path) -> Result<(), InfrastructureError> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() == 0 {
        return Err(InfrastructureError::ValidationError(
            "File is empty".to_string(),
        ));
    }

    let mut file = std::fs::File::open(path)?;
    let mut header = [0u8; 5];
    file.read_exact(&mut header).map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            InfrastructureError::ValidationError(format!(
                "File too small to contain PDF header: {:?}",
                path
            ))
        } else {
            InfrastructureError::ValidationError(format!(
                "Failed to read PDF header from {:?}: {}",
                path, e
            ))
        }
    })?;
    if &header != b"%PDF-" {
        return Err(InfrastructureError::ValidationError(format!(
            "File does not start with %PDF- header: {:?}",
            path
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn create_test_pdf(test_name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("sapo_direct_pdf_tests_{}", test_name));
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

    fn cleanup_test_dir(test_name: &str) {
        let dir = std::env::temp_dir().join(format!("sapo_direct_pdf_tests_{}", test_name));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_direct_pdf_returns_raw_bytes_unchanged() {
        let pdf_path = create_test_pdf("raw_bytes");
        let file_content = std::fs::read(&pdf_path).unwrap();

        let renderer = DirectPdfRenderer::new();
        let config = RenderConfig::default();
        let output = renderer.render(&pdf_path, &config).unwrap();

        assert_eq!(
            output, file_content,
            "Output must be byte-for-byte identical to input"
        );
        cleanup_test_dir("raw_bytes");
    }

    #[test]
    fn test_direct_pdf_preserves_pdf_header() {
        let pdf_path = create_test_pdf("header_check");

        let renderer = DirectPdfRenderer::new();
        let config = RenderConfig::default();
        let output = renderer.render(&pdf_path, &config).unwrap();

        assert!(output.len() >= 5);
        assert_eq!(&output[..5], b"%PDF-", "Output must start with %PDF-");
        cleanup_test_dir("header_check");
    }

    #[test]
    fn test_direct_pdf_invalid_file_returns_validation_error() {
        let dir = std::env::temp_dir().join("sapo_direct_pdf_tests_invalid");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("not_pdf.pdf");
        std::fs::write(&path, b"not a pdf").unwrap();

        let renderer = DirectPdfRenderer::new();
        let config = RenderConfig::default();
        let result = renderer.render(&path, &config);

        assert!(result.is_err());
        match result.unwrap_err() {
            InfrastructureError::ValidationError(_) => {}
            other => panic!("Expected ValidationError, got: {:?}", other),
        }
        cleanup_test_dir("invalid");
    }

    #[test]
    fn test_direct_pdf_missing_file_returns_error() {
        let path = std::path::PathBuf::from("/nonexistent/path/missing.pdf");

        let renderer = DirectPdfRenderer::new();
        let config = RenderConfig::default();
        let result = renderer.render(&path, &config);

        assert!(result.is_err());
        cleanup_test_dir("missing");
    }

    #[test]
    fn test_direct_pdf_empty_file_returns_validation_error() {
        let dir = std::env::temp_dir().join("sapo_direct_pdf_tests_empty");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("empty.pdf");
        std::fs::write(&path, b"").unwrap();

        let renderer = DirectPdfRenderer::new();
        let config = RenderConfig::default();
        let result = renderer.render(&path, &config);

        assert!(result.is_err());
        match result.unwrap_err() {
            InfrastructureError::ValidationError(_) => {}
            other => panic!("Expected ValidationError for empty file, got: {:?}", other),
        }
        cleanup_test_dir("empty");
    }

    #[test]
    fn test_direct_pdf_ignores_config() {
        let pdf_path = create_test_pdf("config_ignore");
        let file_content = std::fs::read(&pdf_path).unwrap();

        let renderer = DirectPdfRenderer::new();

        let configs = vec![
            RenderConfig::default(),
            RenderConfig {
                margin_left_mm: 10.0,
                margin_right_mm: 10.0,
                margin_top_mm: 10.0,
                margin_bottom_mm: 10.0,
                ..Default::default()
            },
            RenderConfig {
                color_mode: crate::infrastructure::renderer::ColorMode::Gray,
                ..Default::default()
            },
            RenderConfig {
                paper_size: crate::infrastructure::renderer::PaperSize::a5(),
                dpi: 150,
                ..Default::default()
            },
        ];

        for (i, config) in configs.iter().enumerate() {
            let output = renderer.render(&pdf_path, config).unwrap();
            assert_eq!(
                output, file_content,
                "Config {} should be ignored — output must match input",
                i
            );
        }
        cleanup_test_dir("config_ignore");
    }

    #[test]
    fn test_direct_pdf_performance() {
        let dir = std::env::temp_dir().join("sapo_direct_pdf_perf");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("perf_100kb.pdf");
        let mut content = b"%PDF-1.4\n".to_vec();
        content.extend(vec![0u8; 100_000]);
        std::fs::write(&path, &content).unwrap();

        let renderer = DirectPdfRenderer::new();
        let config = RenderConfig::default();

        let start = std::time::Instant::now();
        let result = renderer.render(&path, &config);
        let elapsed = start.elapsed();

        assert!(result.is_ok());
        assert!(
            elapsed.as_secs_f64() < 1.0,
            "Render took {:.3}s, expected < 1s",
            elapsed.as_secs_f64()
        );
        cleanup_test_dir("perf");
    }

    #[test]
    fn test_direct_pdf_trait_object_compatible() {
        let renderer: Arc<dyn DocumentRenderer> = Arc::new(DirectPdfRenderer::new());
        let pdf_path = create_test_pdf("trait_compat");

        let config = RenderConfig::default();
        let result = renderer.render(&pdf_path, &config);
        assert!(result.is_ok(), "Must work as Arc<dyn DocumentRenderer>");
        cleanup_test_dir("trait_compat");
    }

    #[test]
    fn test_direct_pdf_default_constructor() {
        let _default = DirectPdfRenderer::default();
        let _new = DirectPdfRenderer::new();
    }
}
