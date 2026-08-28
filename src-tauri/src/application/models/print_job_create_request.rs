#[derive(Debug, Clone)]
pub struct PrintJobCreateRequest {
    pub slip_id: String,
    pub pdf_url: String,
}
