use crate::domain::print_job::aggregate::PrintJob;
use crate::domain::print_job::value_objects::PrintStatus;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_progress_pending() {
        assert_eq!(calculate_progress(&PrintStatus::Pending), 0);
    }

    #[test]
    fn test_calculate_progress_queued() {
        assert_eq!(calculate_progress(&PrintStatus::Queued), 10);
    }

    #[test]
    fn test_calculate_progress_downloaded() {
        assert_eq!(calculate_progress(&PrintStatus::Downloaded), 40);
    }

    #[test]
    fn test_calculate_progress_submitted() {
        assert_eq!(calculate_progress(&PrintStatus::SubmittedToQueue), 60);
    }

    #[test]
    fn test_calculate_progress_printing() {
        assert_eq!(calculate_progress(&PrintStatus::Printing), 80);
    }

    #[test]
    fn test_calculate_progress_completed() {
        assert_eq!(calculate_progress(&PrintStatus::Completed), 100);
    }

    #[test]
    fn test_calculate_progress_failed() {
        assert_eq!(calculate_progress(&PrintStatus::Failed), 0);
    }

    #[test]
    fn test_calculate_progress_cancelled() {
        assert_eq!(calculate_progress(&PrintStatus::Cancelled), 0);
    }

    #[test]
    fn test_status_to_string_all_variants() {
        assert_eq!(status_to_string(&PrintStatus::Pending), "PENDING");
        assert_eq!(status_to_string(&PrintStatus::Queued), "QUEUED");
        assert_eq!(status_to_string(&PrintStatus::Downloaded), "DOWNLOADED");
        assert_eq!(
            status_to_string(&PrintStatus::SubmittedToQueue),
            "SUBMITTED_TO_QUEUE"
        );
        assert_eq!(status_to_string(&PrintStatus::Printing), "PRINTING");
        assert_eq!(status_to_string(&PrintStatus::Completed), "COMPLETED");
        assert_eq!(status_to_string(&PrintStatus::Failed), "FAILED");
        assert_eq!(status_to_string(&PrintStatus::Cancelled), "CANCELLED");
    }

    #[test]
    fn test_job_dto_from_print_job() {
        let job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP_LaserJet".to_string(),
        );

        let dto: JobDto = job.into();

        assert!(!dto.job_id.is_empty());
        assert_eq!(dto.printer_name, "HP_LaserJet");
        assert_eq!(dto.status, "PENDING");
        assert_eq!(dto.progress, 0);
    }
}
