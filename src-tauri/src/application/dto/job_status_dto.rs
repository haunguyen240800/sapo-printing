use serde::{Deserialize, Serialize};

use crate::application::dto::job_dto::calculate_progress;
use crate::domain::models::PrintJob;
use crate::domain::models::PrintStatus;

/// DTO cho status polling — dùng bởi web app polling qua native messaging.
///
/// Bao gồm đầy đủ timestamps và error_message để web app render UI chi tiết.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStatusDto {
    pub job_id: String,
    pub status: String,
    pub progress: u8,       // 0-100%
    pub printer_name: String,
    pub created_at: i64,    // Unix timestamp
    pub updated_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub error_message: Option<String>,
}

impl From<PrintJob> for JobStatusDto {
    fn from(job: PrintJob) -> Self {
        let status = status_to_string(job.status());

        Self {
            job_id: job.id().to_string(),
            status,
            progress: calculate_progress(job.status()),
            printer_name: job.printer_name().to_string(),
            created_at: job.created_at(),
            updated_at: None, // PrintJob doesn't track updated_at yet
            completed_at: job.completed_at(),
            error_message: job.error_message().cloned(),
        }
    }
}

/// Helper: PrintStatus → String cho API response.
pub fn status_to_string(status: &PrintStatus) -> String {
    match status {
        PrintStatus::Pending => "PENDING".to_string(),
        PrintStatus::Queued => "QUEUED".to_string(),
        PrintStatus::Downloaded => "DOWNLOADED".to_string(),
        PrintStatus::SubmittedToQueue => "SUBMITTED_TO_QUEUE".to_string(),
        PrintStatus::Printing => "PRINTING".to_string(),
        PrintStatus::Completed => "COMPLETED".to_string(),
        PrintStatus::Failed => "FAILED".to_string(),
        PrintStatus::Cancelled => "CANCELLED".to_string(),
    }
}


