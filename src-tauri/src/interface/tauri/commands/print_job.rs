use crate::application::dto::cancel_job_request::CancelJobRequest;
use crate::application::dto::create_job_request::CreateJobRequest;
use crate::application::dto::{JobDto, JobFilterDto};
use crate::application::use_cases::cancel_print_job::CancelPrintJobUseCase;
use crate::application::use_cases::create_print_job::CreatePrintJobUseCase;
use crate::application::use_cases::errors::ApplicationError;
use crate::application::use_cases::list_jobs::ListJobsUseCase;
use crate::domain::print_job::value_objects::JobId;
use crate::AppContextState;
use std::str::FromStr;

/// Payload received from the UI for creating a print job.
#[derive(serde::Deserialize)]
pub struct CreateJobPayload {
    pub pdf_urls: Vec<String>,
    pub printer_name: String,
    /// Optional output path for "Print to PDF" printers.
    /// If provided, file will be saved to this path instead of auto-generated name.
    pub output_path: Option<String>,
}

/// Payload received from the UI for cancelling a print job.
#[derive(serde::Deserialize)]
pub struct CancelJobPayload {
    pub job_id: String,
}

/// Execute the create print job use case and map results to Tauri-compatible types.
/// Called from the `create_print_job` Tauri command in `main.rs`.
pub fn execute_create_print_job(
    payload: CreateJobPayload,
    ctx: &AppContextState,
) -> Result<Vec<String>, String> {
    let use_case = CreatePrintJobUseCase {
        job_repo: ctx.job_repo.clone(),
        event_store: ctx.event_store.clone(),
        event_bus: ctx.event_bus.clone(),
        printer_manager: ctx.printer_manager.clone(),
    };

    let request = CreateJobRequest {
        pdf_urls: payload.pdf_urls,
        printer_name: payload.printer_name,
        output_path: payload.output_path,
    };

    use_case
        .execute(request)
        .map(|ids| ids.iter().map(|id| id.to_string()).collect())
        .map_err(|e| match &e {
            ApplicationError::EmptyJobList => "Danh sách URLs không được rỗng".to_string(),
            ApplicationError::TooManyJobs { count } => format!(
                "Số lượng URLs vượt quá giới hạn 5000 (nhận được: {})",
                count
            ),
            ApplicationError::PrinterNotAvailable { name } => {
                format!("Máy in '{}' không khả dụng hoặc đang offline", name)
            }
            _ => format!("{}", e),
        })
}

/// Execute the cancel print job use case and map results to Tauri-compatible types.
/// Called from the `cancel_print_job` Tauri command in `main.rs`.
pub fn execute_cancel_print_job(
    payload: CancelJobPayload,
    ctx: &AppContextState,
) -> Result<(), String> {
    let use_case = CancelPrintJobUseCase::new(
        ctx.job_repo.clone(),
        ctx.event_store.clone(),
        ctx.event_bus.clone(),
    );

    let request = CancelJobRequest {
        job_id: payload.job_id,
    };

    use_case.execute(request).map_err(|e| match &e {
        ApplicationError::InvalidJobId { job_id } => {
            format!("Job ID không hợp lệ: {}", job_id)
        }
        ApplicationError::JobNotFound { job_id } => {
            format!("Không tìm thấy job với ID: {}", job_id)
        }
        ApplicationError::CannotCancelCompleted { job_id } => {
            format!("Không thể hủy job đã hoàn thành: {}", job_id)
        }
        ApplicationError::CannotCancelFailed { job_id } => {
            format!("Không thể hủy job đã thất bại: {}", job_id)
        }
        ApplicationError::CannotCancelCancelled { job_id } => {
            format!("Job đã bị hủy trước đó: {}", job_id)
        }
        _ => format!("{}", e),
    })
}

/// Execute the list jobs use case with filtering.
/// Called from the `list_jobs` Tauri command in `main.rs`.
pub fn execute_list_jobs(
    filter: JobFilterDto,
    ctx: &AppContextState,
) -> Result<Vec<JobDto>, String> {
    let use_case = ListJobsUseCase::new(ctx.job_repo.clone());

    use_case
        .execute(filter)
        .map_err(|e| format!("Lấy danh sách job thất bại: {:?}", e))
}

/// Execute get job status by ID.
/// Called from the `get_job_status` Tauri command in `main.rs`.
pub fn execute_get_job_status(job_id: String, ctx: &AppContextState) -> Result<JobDto, String> {
    let job_id = JobId::from_str(&job_id)
        .map_err(|_| format!("Job ID không hợp lệ: {}", job_id))?;

    let job = ctx
        .job_repo
        .find_by_id(&job_id)
        .map_err(|e| format!("Lỗi khi lấy job: {:?}", e))?
        .ok_or_else(|| format!("Không tìm thấy job: {}", job_id))?;

    Ok(job.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_job_payload_deserializes() {
        let json = r#"{
            "pdf_urls": ["https://s3.example.com/doc.pdf"],
            "printer_name": "HP_Test"
        }"#;
        let payload: CreateJobPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.pdf_urls.len(), 1);
        assert_eq!(payload.printer_name, "HP_Test");
    }

    #[test]
    fn test_cancel_job_payload_deserializes() {
        let json = r#"{
            "job_id": "12345678-1234-1234-1234-123456789abc"
        }"#;
        let payload: CancelJobPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.job_id, "12345678-1234-1234-1234-123456789abc");
    }
}
