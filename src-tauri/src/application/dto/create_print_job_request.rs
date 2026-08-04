/// Input DTO cho CreatePrintJobUseCase.
#[derive(Debug, Clone)]
pub struct CreatePrintJobRequest {
    /// S3 PDF URL của tài liệu cần in
    pub pdf_url: String,
}
