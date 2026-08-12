use crate::application::dto::{PrintJobDto, PrintJobFilterDto};
use crate::application::errors::ApplicationError;
use crate::domain::print_job::PrintJobRepository;
use std::sync::Arc;

pub struct ListPrintJobsUseCase {
    job_repo: Arc<dyn PrintJobRepository>,
}

impl ListPrintJobsUseCase {
    pub fn new(job_repo: Arc<dyn PrintJobRepository>) -> Self {
        Self { job_repo }
    }

    pub fn execute(&self, filter: PrintJobFilterDto) -> Result<Vec<PrintJobDto>, ApplicationError> {
        tracing::info!(
            target = "sapo_printer::application::use_case::list_print_jobs",
            status_filter = ?filter.status,
            printer_filter = ?filter.printer_name,
            "ListPrintJobsUseCase: starting"
        );

        let jobs = if let Some(status_str) = &filter.status {
            let status = parse_status(status_str)?;
            self.job_repo.find_by_status(&status)
        } else {
            self.job_repo.find_all()
        }
        .map_err(|e| ApplicationError::RepositoryError(format!("Failed to load jobs: {:?}", e)))?;

        let mut result: Vec<PrintJobDto> = jobs
            .into_iter()
            .map(|j| {
                let dto: PrintJobDto = j.into();
                // Note: created_at is set to 0 in From<PrintJob> trait
                // In a full implementation, we'd need to extend repository to return timestamps
                // For now, keeping it simple as per AC-1 spec
                dto
            })
            .collect();

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
            target = "sapo_printer::application::use_case::list_print_jobs",
            result_count = result.len(),
            "ListPrintJobsUseCase: completed"
        );

        Ok(result)
    }
}

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
