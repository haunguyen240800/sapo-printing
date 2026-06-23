use crate::application::dto::create_job_request::CreateJobRequest;
use crate::application::use_cases::create_print_job::CreatePrintJobUseCase;
use crate::application::use_cases::errors::ApplicationError;
use crate::AppContextState;

/// Payload received from the UI for creating a print job.
#[derive(serde::Deserialize)]
pub struct CreateJobPayload {
    pub pdf_urls: Vec<String>,
    pub printer_name: String,
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
        printer_repo: ctx.printer_repo.clone(),
    };

    let request = CreateJobRequest {
        pdf_urls: payload.pdf_urls,
        printer_name: payload.printer_name,
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
}
