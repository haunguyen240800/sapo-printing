use serde::{Deserialize, Serialize};

use crate::application::dto::job_dto::calculate_progress;
use crate::domain::print_job::PrintJob;
use crate::domain::print_job::PrintStatus;

/// DTO cho status polling â€” dĂ¹ng bá»Ÿi web app polling qua native messaging.
///
/// Bao gá»“m Ä‘áº§y Ä‘á»§ timestamps vĂ  error_message Ä‘á»ƒ web app render UI chi tiáº¿t.
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

/// Helper: PrintStatus â†’ String cho API response.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_status_dto_from_print_job_pending() {
        let job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP_LaserJet".to_string(),
        );

        let dto: JobStatusDto = job.into();

        assert!(!dto.job_id.is_empty());
        assert_eq!(dto.printer_name, "HP_LaserJet");
        assert_eq!(dto.status, "PENDING");
        assert_eq!(dto.progress, 0);
        assert!(dto.updated_at.is_none());
        assert!(dto.completed_at.is_none());
        assert!(dto.error_message.is_none());
    }

    #[test]
    fn test_job_status_dto_from_print_job_completed() {
        let mut job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP_LaserJet".to_string(),
        );
        job.queue().unwrap();
        job.mark_downloaded().unwrap();
        job.mark_submitted().unwrap();
        job.mark_printing().unwrap();
        job.complete().unwrap();

        let dto: JobStatusDto = job.into();

        assert_eq!(dto.status, "COMPLETED");
        assert_eq!(dto.progress, 100);
        assert!(dto.completed_at.is_some());
    }

    #[test]
    fn test_job_status_dto_from_print_job_failed() {
        let mut job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP_LaserJet".to_string(),
        );
        job.queue().unwrap();
        job.fail("Download timeout".to_string()).unwrap();

        let dto: JobStatusDto = job.into();

        assert_eq!(dto.status, "FAILED");
        assert_eq!(dto.progress, 0);
        assert!(dto.completed_at.is_some());
        assert_eq!(dto.error_message, Some("Download timeout".to_string()));
    }

    #[test]
    fn test_job_status_dto_from_print_job_cancelled() {
        let mut job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP_LaserJet".to_string(),
        );
        job.cancel().unwrap();

        let dto: JobStatusDto = job.into();

        assert_eq!(dto.status, "CANCELLED");
        assert_eq!(dto.progress, 0);
        assert!(dto.completed_at.is_some());
    }

    #[test]
    fn test_job_status_dto_progress_all_states() {
        let cases = [
            (PrintStatus::Pending, 0),
            (PrintStatus::Queued, 10),
            (PrintStatus::Downloaded, 40),
            (PrintStatus::SubmittedToQueue, 60),
            (PrintStatus::Printing, 80),
            (PrintStatus::Completed, 100),
            (PrintStatus::Failed, 0),
            (PrintStatus::Cancelled, 0),
        ];

        for (status, expected_progress) in cases {
            assert_eq!(
                calculate_progress(&status),
                expected_progress,
                "Progress mismatch for {:?}: expected {}, got {}",
                status,
                expected_progress,
                calculate_progress(&status)
            );
        }
    }

    #[test]
    fn test_status_to_string_all_variants() {
        assert_eq!(status_to_string(&PrintStatus::Pending), "PENDING");
        assert_eq!(status_to_string(&PrintStatus::Queued), "QUEUED");
        assert_eq!(status_to_string(&PrintStatus::Downloaded), "DOWNLOADED");
        assert_eq!(status_to_string(&PrintStatus::SubmittedToQueue), "SUBMITTED_TO_QUEUE");
        assert_eq!(status_to_string(&PrintStatus::Printing), "PRINTING");
        assert_eq!(status_to_string(&PrintStatus::Completed), "COMPLETED");
        assert_eq!(status_to_string(&PrintStatus::Failed), "FAILED");
        assert_eq!(status_to_string(&PrintStatus::Cancelled), "CANCELLED");
    }
}
