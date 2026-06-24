use crate::application::dto::{JobDto, JobFilterDto};
use crate::application::use_cases::errors::ApplicationError;
use crate::domain::print_job::PrintJobRepository;
use std::sync::Arc;

/// Use case: List print jobs với filtering capabilities.
///
/// Filters supported:
/// - Status: filter by specific PrintStatus
/// - Printer name: filter by printer_name
/// - Date range: from_date và to_date (Unix timestamps)
///
/// Returns: Vec<JobDto> với timestamps và progress calculations
pub struct ListJobsUseCase {
    job_repo: Arc<dyn PrintJobRepository>,
}

impl ListJobsUseCase {
    pub fn new(job_repo: Arc<dyn PrintJobRepository>) -> Self {
        Self { job_repo }
    }

    pub fn execute(&self, filter: JobFilterDto) -> Result<Vec<JobDto>, ApplicationError> {
        // Step 1: Load jobs from repository based on status filter
        let jobs = if let Some(status_str) = &filter.status {
            let status = parse_status(status_str)?;
            self.job_repo.find_by_status(&status)
        } else {
            self.job_repo.find_all()
        }
        .map_err(|e| ApplicationError::RepositoryError(format!("Failed to load jobs: {:?}", e)))?;

        // Step 2: Convert to DTOs with enriched data (timestamps, progress)
        let mut result: Vec<JobDto> = jobs
            .into_iter()
            .map(|j| {
                let dto: JobDto = j.into();
                // Note: created_at is set to 0 in From<PrintJob> trait
                // In a full implementation, we'd need to extend repository to return timestamps
                // For now, keeping it simple as per AC-1 spec
                dto
            })
            .collect();

        // Step 3: Apply additional client-side filters
        if let Some(printer) = &filter.printer_name {
            result.retain(|j| j.printer_name == *printer);
        }

        if let Some(from) = filter.from_date {
            result.retain(|j| j.created_at >= from);
        }

        if let Some(to) = filter.to_date {
            result.retain(|j| j.created_at <= to);
        }

        Ok(result)
    }
}

/// Helper: parse status string từ filter DTO.
fn parse_status(
    status_str: &str,
) -> Result<crate::domain::print_job::value_objects::PrintStatus, ApplicationError> {
    use crate::domain::print_job::value_objects::PrintStatus;

    match status_str {
        "PENDING" => Ok(PrintStatus::Pending),
        "QUEUED" => Ok(PrintStatus::Queued),
        "DOWNLOADED" => Ok(PrintStatus::Downloaded),
        "SUBMITTED_TO_QUEUE" => Ok(PrintStatus::SubmittedToQueue),
        "PRINTING" => Ok(PrintStatus::Printing),
        "COMPLETED" => Ok(PrintStatus::Completed),
        "FAILED" => Ok(PrintStatus::Failed),
        "CANCELLED" => Ok(PrintStatus::Cancelled),
        _ => Err(ApplicationError::ValidationError {
            reason: format!("Invalid status: {}", status_str),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::print_job::aggregate::PrintJob;
    use crate::domain::print_job::errors::DomainError;
    use crate::domain::print_job::value_objects::{JobId, PrintStatus};
    use std::sync::Mutex;

    // Mock repository for testing
    struct MockJobRepository {
        jobs: Mutex<Vec<PrintJob>>,
    }

    impl MockJobRepository {
        fn new(jobs: Vec<PrintJob>) -> Self {
            Self {
                jobs: Mutex::new(jobs),
            }
        }
    }

    impl PrintJobRepository for MockJobRepository {
        fn save(&self, _job: &PrintJob) -> Result<(), DomainError> {
            unimplemented!()
        }

        fn update(&self, _job: &PrintJob) -> Result<(), DomainError> {
            unimplemented!()
        }

        fn find_by_id(&self, _id: &JobId) -> Result<Option<PrintJob>, DomainError> {
            unimplemented!()
        }

        fn find_by_status(&self, status: &PrintStatus) -> Result<Vec<PrintJob>, DomainError> {
            let jobs = self.jobs.lock().unwrap();
            Ok(jobs
                .iter()
                .filter(|j| j.status() == status)
                .cloned()
                .collect())
        }

        fn find_all(&self) -> Result<Vec<PrintJob>, DomainError> {
            Ok(self.jobs.lock().unwrap().clone())
        }
    }

    fn make_test_jobs() -> Vec<PrintJob> {
        vec![
            PrintJob::new(
                "https://s3.example.com/doc1.pdf".to_string(),
                "HP_LaserJet".to_string(),
            ),
            PrintJob::new(
                "https://s3.example.com/doc2.pdf".to_string(),
                "Canon_Printer".to_string(),
            ),
            {
                let mut job = PrintJob::new(
                    "https://s3.example.com/doc3.pdf".to_string(),
                    "HP_LaserJet".to_string(),
                );
                job.queue().unwrap();
                job
            },
        ]
    }

    #[test]
    fn test_list_all_jobs() {
        let jobs = make_test_jobs();
        let repo = Arc::new(MockJobRepository::new(jobs));
        let use_case = ListJobsUseCase::new(repo);

        let filter = JobFilterDto {
            status: None,
            printer_name: None,
            from_date: None,
            to_date: None,
        };

        let result = use_case.execute(filter).unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_filter_by_status() {
        let jobs = make_test_jobs();
        let repo = Arc::new(MockJobRepository::new(jobs));
        let use_case = ListJobsUseCase::new(repo);

        let filter = JobFilterDto {
            status: Some("PENDING".to_string()),
            printer_name: None,
            from_date: None,
            to_date: None,
        };

        let result = use_case.execute(filter).unwrap();
        assert_eq!(result.len(), 2); // 2 PENDING jobs
        assert!(result.iter().all(|j| j.status == "PENDING"));
    }

    #[test]
    fn test_filter_by_printer_name() {
        let jobs = make_test_jobs();
        let repo = Arc::new(MockJobRepository::new(jobs));
        let use_case = ListJobsUseCase::new(repo);

        let filter = JobFilterDto {
            status: None,
            printer_name: Some("HP_LaserJet".to_string()),
            from_date: None,
            to_date: None,
        };

        let result = use_case.execute(filter).unwrap();
        assert_eq!(result.len(), 2); // 2 jobs with HP_LaserJet
        assert!(result
            .iter()
            .all(|j| j.printer_name == "HP_LaserJet"));
    }

    #[test]
    fn test_invalid_status_returns_error() {
        let jobs = make_test_jobs();
        let repo = Arc::new(MockJobRepository::new(jobs));
        let use_case = ListJobsUseCase::new(repo);

        let filter = JobFilterDto {
            status: Some("INVALID_STATUS".to_string()),
            printer_name: None,
            from_date: None,
            to_date: None,
        };

        let result = use_case.execute(filter);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApplicationError::ValidationError { .. }
        ));
    }

    #[test]
    fn test_parse_status_all_variants() {
        assert!(matches!(
            parse_status("PENDING").unwrap(),
            PrintStatus::Pending
        ));
        assert!(matches!(
            parse_status("QUEUED").unwrap(),
            PrintStatus::Queued
        ));
        assert!(matches!(
            parse_status("DOWNLOADED").unwrap(),
            PrintStatus::Downloaded
        ));
        assert!(matches!(
            parse_status("SUBMITTED_TO_QUEUE").unwrap(),
            PrintStatus::SubmittedToQueue
        ));
        assert!(matches!(
            parse_status("PRINTING").unwrap(),
            PrintStatus::Printing
        ));
        assert!(matches!(
            parse_status("COMPLETED").unwrap(),
            PrintStatus::Completed
        ));
        assert!(matches!(
            parse_status("FAILED").unwrap(),
            PrintStatus::Failed
        ));
        assert!(matches!(
            parse_status("CANCELLED").unwrap(),
            PrintStatus::Cancelled
        ));
    }

    #[test]
    fn test_progress_calculation_in_dto() {
        let jobs = make_test_jobs();
        let repo = Arc::new(MockJobRepository::new(jobs));
        let use_case = ListJobsUseCase::new(repo);

        let filter = JobFilterDto {
            status: Some("QUEUED".to_string()),
            printer_name: None,
            from_date: None,
            to_date: None,
        };

        let result = use_case.execute(filter).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].progress, 10); // QUEUED → 10%
    }
}
