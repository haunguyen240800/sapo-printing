use crate::AppContextState;
use crate::application::dto::cancel_print_job_request::CancelPrintJobRequest;
use crate::application::dto::create_print_job_request::CreatePrintJobRequest;
use crate::application::dto::{PrintJobDto, PrintJobFilterDto};
use crate::application::errors::ApplicationError;
use crate::application::use_cases::cancel_print_job::CancelPrintJobUseCase;
use crate::application::use_cases::create_print_job::CreatePrintJobUseCase;
use crate::application::use_cases::list_print_jobs::ListPrintJobsUseCase;
use crate::domain::print_job::PrintJobId;
use std::str::FromStr;

#[derive(serde::Deserialize)]
pub struct CreateJobPayload {
    pub pdf_urls: Vec<String>,
    pub output_path: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct CancelJobPayload {
    pub job_id: String,
}

pub fn execute_create_print_job(
    payload: CreateJobPayload,
    ctx: &AppContextState,
) -> Result<Vec<String>, String> {
    if payload.pdf_urls.is_empty() {
        return Err("Danh sách URLs không được rỗng".to_string());
    }
    if payload.pdf_urls.len() > 5000 {
        return Err(format!(
            "Số lượng URLs vượt quá giới hạn 5000 (nhận được: {})",
            payload.pdf_urls.len()
        ));
    }

    let use_case = CreatePrintJobUseCase {
        job_repo: ctx.job_repo.clone(),
        event_store: ctx.event_store.clone(),
        event_bus: ctx.event_bus.clone(),
        config_provider: ctx.config_provider.clone(),
        printer_manager: ctx.printer_manager.clone(),
    };

    let mut job_ids = Vec::with_capacity(payload.pdf_urls.len());
    for url in payload.pdf_urls {
        let request = CreatePrintJobRequest { pdf_url: url };
        let id = use_case.execute(request).map_err(|e| match &e {
            ApplicationError::PrinterNotAvailable { name } => {
                format!("Máy in ‘{}’ không khả dụng hoặc đang offline", name)
            }
            _ => format!("{}", e),
        })?;
        job_ids.push(id.to_string());
    }

    Ok(job_ids)
}

pub fn execute_cancel_print_job(
    payload: CancelJobPayload,
    ctx: &AppContextState,
) -> Result<(), String> {
    let use_case = CancelPrintJobUseCase::new(
        ctx.job_repo.clone(),
        ctx.event_store.clone(),
        ctx.event_bus.clone(),
        ctx.temp_files.clone(),
    );

    let request = CancelPrintJobRequest {
        job_id: payload.job_id,
    };

    use_case.execute(request).map_err(|e| match &e {
        ApplicationError::InvalidJobId { job_id } => {
            format!("Job ID khĂ´ng há»£p lá»‡: {}", job_id)
        }
        ApplicationError::JobNotFound { job_id } => {
            format!("KhĂ´ng tĂ¬m tháº¥y job vá»›i ID: {}", job_id)
        }
        ApplicationError::CannotCancelCompleted { job_id } => {
            format!("KhĂ´ng thá»ƒ há»§y job Ä‘Ă£ hoĂ n thĂ nh: {}", job_id)
        }
        ApplicationError::CannotCancelFailed { job_id } => {
            format!("KhĂ´ng thá»ƒ há»§y job Ä‘Ă£ tháº¥t báº¡i: {}", job_id)
        }
        ApplicationError::CannotCancelCancelled { job_id } => {
            format!("Job Ä‘Ă£ bá»‹ há»§y trÆ°á»›c Ä‘Ă³: {}", job_id)
        }
        _ => format!("{}", e),
    })
}

pub fn execute_list_jobs(
    filter: PrintJobFilterDto,
    ctx: &AppContextState,
) -> Result<Vec<PrintJobDto>, String> {
    let use_case = ListPrintJobsUseCase::new(ctx.job_repo.clone());

    use_case
        .execute(filter)
        .map_err(|e| format!("Láº¥y danh sĂ¡ch job tháº¥t báº¡i: {:?}", e))
}

pub fn execute_get_job_status(
    job_id: String,
    ctx: &AppContextState,
) -> Result<PrintJobDto, String> {
    let job_id = PrintJobId::from_str(&job_id)
        .map_err(|_| format!("Job ID khĂ´ng há»£p lá»‡: {}", job_id))?;

    let job = ctx
        .job_repo
        .find_by_id(&job_id)
        .map_err(|e| format!("Lá»—i khi láº¥y job: {:?}", e))?
        .ok_or_else(|| format!("KhĂ´ng tĂ¬m tháº¥y job: {}", job_id))?;

    Ok(job.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_job_payload_deserializes() {
        let json = r#"{
            "pdf_urls": ["https://s3.example.com/doc1.pdf", "https://s3.example.com/doc2.pdf"]
        }"#;
        let payload: CreateJobPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.pdf_urls.len(), 2);
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
