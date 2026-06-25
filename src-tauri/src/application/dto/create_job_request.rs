/// Input DTO cho CreatePrintJobUseCase.
///
/// Validation: 1–5000 URLs, printer_name không rỗng.
#[derive(Debug, Clone)]
pub struct CreateJobRequest {
    /// Danh sách S3 PDF URLs (1–5000)
    pub pdf_urls: Vec<String>,
    /// Tên máy in (phải match printer đang ONLINE)
    pub printer_name: String,
    /// Optional output path for "Print to PDF" printers
    pub output_path: Option<String>,
}
