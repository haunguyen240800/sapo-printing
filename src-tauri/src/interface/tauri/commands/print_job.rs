use crate::AppContextState;
use crate::application::dto::cancel_print_job_request::CancelPrintJobRequest;
use crate::application::dto::print_job_status_dto::PrintJobStatusDto;
use crate::application::dto::{PrintJobDto, PrintJobFilterDto};
use crate::application::errors::Error;

#[derive(serde::Deserialize)]
pub struct CreateJobPayload {
    pub pdf_urls: Vec<String>,
    pub output_path: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct CancelJobPayload {
    pub job_id: String,
}

pub fn execute_cancel_print_job(
    payload: CancelJobPayload,
    ctx: &AppContextState,
) -> Result<(), String> {
    let use_case = ctx.cancel_print_job_uc.clone();

    let request = CancelPrintJobRequest {
        job_id: payload.job_id,
    };

    use_case.execute(request).map_err(|e| match &e {
        Error::InvalidJobId { job_id } => {
            format!("Job ID khĂ´ng há»£p lá»‡: {}", job_id)
        }
        Error::JobNotFound { job_id } => {
            format!("KhĂ´ng tĂ¬m tháº¥y job vá»›i ID: {}", job_id)
        }
        Error::CannotCancelCompleted { job_id } => {
            format!("KhĂ´ng thá»ƒ há»§y job Ä‘Ă£ hoĂ n thĂ nh: {}", job_id)
        }
        Error::CannotCancelFailed { job_id } => {
            format!("KhĂ´ng thá»ƒ há»§y job Ä‘Ă£ tháº¥t báº¡i: {}", job_id)
        }
        Error::CannotCancelCancelled { job_id } => {
            format!("Job Ä‘Ă£ bá»‹ há»§y trÆ°á»›c Ä‘Ă³: {}", job_id)
        }
        _ => format!("{}", e),
    })
}

pub fn execute_list_jobs(
    filter: PrintJobFilterDto,
    ctx: &AppContextState,
) -> Result<Vec<PrintJobDto>, String> {
    let use_case = ctx.list_print_jobs_uc.clone();

    use_case
        .execute(filter)
        .map_err(|e| format!("Láº¥y danh sĂ¡ch job tháº¥t báº¡i: {:?}", e))
}

pub fn execute_get_job_status(
    job_id: String,
    ctx: &AppContextState,
) -> Result<PrintJobStatusDto, String> {
    let use_case = ctx.get_job_status_uc.clone();

    use_case.execute(&job_id).map_err(|e| match &e {
        Error::InvalidJobId { job_id } => format!("Job ID khĂ´ng há»£p lá»‡: {}", job_id),
        Error::JobNotFound { job_id } => format!("KhĂ´ng tĂ¬m tháº¥y job: {}", job_id),
        _ => format!("{}", e),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cancel_job_payload_deserializes() {
        let json = r#"{
            "job_id": "12345678-1234-1234-1234-123456789abc"
        }"#;
        let payload: CancelJobPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload.job_id, "12345678-1234-1234-1234-123456789abc");
    }
}
