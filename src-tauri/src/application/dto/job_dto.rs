use crate::domain::models::PrintJob;
use crate::domain::models::PrintStatus;
use serde::{Deserialize, Serialize};

/// DTO cho danh sách print jobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDto {
    pub job_id: String,
    pub printer_name: String,
    pub status: String,
    pub progress: u8, // 0-100%
    pub created_at: i64,
    pub error_message: Option<String>,
}

/// DTO cho filtering danh sách jobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobFilterDto {
    pub status: Option<String>,
    pub printer_name: Option<String>,
    pub from_date: Option<i64>, // Unix timestamp
    pub to_date: Option<i64>,
}

impl From<PrintJob> for JobDto {
    fn from(job: PrintJob) -> Self {
        Self {
            job_id: job.id().to_string(),
            printer_name: job.printer_name().to_string(),
            status: status_to_string(job.status()),
            progress: calculate_progress(job.status()),
            created_at: job.created_at(),
            error_message: job.error_message().cloned(),
        }
    }
}

/// Helper: tính progress percentage dựa trên status.
pub(crate) fn calculate_progress(status: &PrintStatus) -> u8 {
    match status {
        PrintStatus::Pending => 0,
        PrintStatus::Queued => 10,
        PrintStatus::Downloaded => 40,
        PrintStatus::SubmittedToQueue => 60,
        PrintStatus::Printing => 80,
        PrintStatus::Completed => 100,
        PrintStatus::Failed | PrintStatus::Cancelled => 0,
    }
}

/// Helper: PrintStatus → String cho API response.
fn status_to_string(status: &PrintStatus) -> String {
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


