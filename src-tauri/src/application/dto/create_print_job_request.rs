/// Input DTO cho CreatePrintJobUseCase.
///
/// Validation: 1–5000 URLs.
#[derive(Debug, Clone)]
pub struct CreatePrintJobRequest {
    /// Danh sách S3 PDF URLs (1–5000)
    pub pdf_urls: Vec<String>,
    /// Optional output path for "Print to PDF" printers
    pub output_path: Option<String>,
}
