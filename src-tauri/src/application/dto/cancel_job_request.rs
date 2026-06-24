/// Request DTO for CancelPrintJobUseCase.
///
/// Validation: job_id must be valid UUID format.
#[derive(Debug, Clone)]
pub struct CancelJobRequest {
    /// Job ID to cancel (UUID string format)
    pub job_id: String,
}
