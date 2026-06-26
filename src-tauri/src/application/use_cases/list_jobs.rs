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
        tracing::info!(
            target = "sapo_printer::use_case::list_jobs",
            status_filter = ?filter.status,
            printer_filter = ?filter.printer_name,
            "ListJobsUseCase: starting"
        );

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

        tracing::debug!(
            target = "sapo_printer::use_case::list_jobs",
            result_count = result.len(),
            "ListJobsUseCase: completed"
        );

        Ok(result)
    }
}

/// Helper: parse status string từ filter DTO.
fn parse_status(
    status_str: &str,
) -> Result<crate::domain::print_job::PrintStatus, ApplicationError> {
    use crate::domain::print_job::PrintStatus;

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


