use std::io::{self, Read, Write};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::application::dto::create_job_request::CreateJobRequest;
use crate::application::dto::cancel_job_request::CancelJobRequest;
use crate::application::use_cases::cancel_print_job::CancelPrintJobUseCase;
use crate::application::use_cases::create_print_job::CreatePrintJobUseCase;
use crate::application::use_cases::errors::ApplicationError;
use crate::application::use_cases::get_job_status::GetJobStatusUseCase;
use crate::domain::print_job::PrintJobRepository;
use crate::infrastructure::database::SqliteEventStore;

use crate::interface::tauri::dtos::printer_dto::PrinterDto;
use crate::shared::event_bus::EventBus;

const MAX_MESSAGE_SIZE: u32 = 1_048_576; // 1 MB

// â”€â”€ Wire Protocol â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[derive(Debug)]
pub enum ProtocolError {
    Io(io::Error),
    MessageTooLarge { size: u32 },
    InvalidUtf8,
    Eof,
    /// Length header read but body arrived at EOF before full payload.
    PartialMessage { expected: u32, received: usize },
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolError::Io(e) => write!(f, "IO error: {}", e),
            ProtocolError::MessageTooLarge { size } => write!(f, "Message too large: {} bytes", size),
            ProtocolError::InvalidUtf8 => write!(f, "Invalid UTF-8 in message"),
            ProtocolError::Eof => write!(f, "End of input"),
            ProtocolError::PartialMessage { expected, received } => {
                write!(f, "Partial message: expected {} bytes, received {}", expected, received)
            }
        }
    }
}

impl From<io::Error> for ProtocolError {
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            ProtocolError::Eof
        } else {
            ProtocolError::Io(e)
        }
    }
}

pub fn read_message(reader: &mut impl Read) -> Result<String, ProtocolError> {
    let mut len_buf = [0u8; 4];
    match reader.read_exact(&mut len_buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Err(ProtocolError::Eof),
        Err(e) => return Err(ProtocolError::Io(e)),
    }
    let msg_len = u32::from_le_bytes(len_buf);

    if msg_len > MAX_MESSAGE_SIZE {
        return Err(ProtocolError::MessageTooLarge { size: msg_len });
    }

    let mut buf = vec![0u8; msg_len as usize];
    match reader.read_exact(&mut buf) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
            // Body truncated â€” partial message. Report expected size.
            return Err(ProtocolError::PartialMessage { expected: msg_len, received: 0 });
        }
        Err(e) => return Err(ProtocolError::Io(e)),
    }

    String::from_utf8(buf).map_err(|_| ProtocolError::InvalidUtf8)
}

pub fn write_message(writer: &mut impl Write, message: &str) -> Result<(), ProtocolError> {
    let len = message.len() as u32;
    if len > MAX_MESSAGE_SIZE {
        return Err(ProtocolError::MessageTooLarge { size: len });
    }
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(message.as_bytes())?;
    writer.flush()?;
    Ok(())
}

#[cfg(windows)]
pub fn set_binary_mode() {
    // Rust's std::io::stdin/stdout use Windows ReadFile/WriteFile directly,
    // bypassing the C runtime text-mode CRLF translation. No explicit binary
    // mode switch is needed.
}

#[cfg(not(windows))]
pub fn set_binary_mode() {
    // Unix stdin/stdout are binary by default
}

// â”€â”€ Origin Validation â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

const ALLOWED_ORIGINS: &[&str] = &[];

pub fn parse_origin() -> Option<String> {
    std::env::args().nth(1).filter(|arg| arg.starts_with("chrome-extension://"))
}

pub fn validate_origin(origin: &Option<String>) -> bool {
    if ALLOWED_ORIGINS.is_empty() {
        // Dev mode: accept all origins. WARNING: in production this must be
        // populated with specific extension IDs, otherwise any extension can
        // communicate with this host.
        return true;
    }
    match origin {
        Some(o) => ALLOWED_ORIGINS.iter().any(|allowed| o == allowed),
        None => false,
    }
}

// â”€â”€ JSON Command Types â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

#[derive(Deserialize)]
struct RawMessage {
    command: Option<String>,
    #[serde(default)]
    pdf_urls: Option<Vec<String>>,
    #[serde(default)]
    printer_name: Option<String>,
    #[serde(default)]
    job_id: Option<String>,
}

#[derive(Serialize)]
struct SuccessResponse<T: Serialize> {
    success: bool,
    data: T,
}

#[derive(Serialize)]
struct ErrorResponse {
    success: bool,
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    code: String,
    message: String,
}

#[derive(Serialize)]
struct PingData {
    status: String,
    version: String,
}

#[derive(Serialize)]
struct PrintBatchData {
    job_ids: Vec<String>,
}

#[derive(Serialize)]
struct CancelData {
    cancelled: bool,
}

#[derive(Serialize)]
struct ListPrintersData {
    printers: Vec<PrinterDto>,
}

// â”€â”€ NativeMessageHandler â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

pub struct NativeMessageHandler {
    pub job_repo: Arc<dyn PrintJobRepository>,
    pub event_store: Arc<SqliteEventStore>,
    pub event_bus: Arc<dyn EventBus>,
    pub metrics_collector: Arc<crate::infrastructure::metrics::MetricsCollector>,
}

impl NativeMessageHandler {
    pub fn new(
        job_repo: Arc<dyn PrintJobRepository>,
        event_store: Arc<SqliteEventStore>,
        event_bus: Arc<dyn EventBus>,
        metrics_collector: Arc<crate::infrastructure::metrics::MetricsCollector>,
    ) -> Self {
        Self {
            job_repo,            event_store,
            event_bus,
            metrics_collector,
        }
    }

    pub fn handle_message(&self, raw: &str) -> String {
        let msg: RawMessage = match serde_json::from_str(raw) {
            Ok(m) => m,
            Err(_) => return self.error_response("INVALID_REQUEST", "Invalid JSON"),
        };

        let command = match &msg.command {
            Some(c) => c.clone(),
            None => return self.error_response("INVALID_REQUEST", "Missing command field"),
        };

        tracing::info!(
            target = "sapo_printer::native_messaging",
            command = command,
            "NativeMessagingHandler: message received"
        );

        let result = match command.as_str() {
            "ping" => self.handle_ping(),
            "print_batch" => self.handle_print_batch(msg),
            "get_status" => self.handle_get_status(msg),
            "cancel_job" => self.handle_cancel_job(msg),
            "list_printers" => self.handle_list_printers(),
            unknown => self.error_response("UNKNOWN_COMMAND", &format!("Unknown command: {}", unknown)),
        };

        tracing::debug!(
            target = "sapo_printer::native_messaging",
            command = command,
            response_truncated = result.chars().take(200).collect::<String>(),
            "NativeMessagingHandler: response sent"
        );

        result
    }

    fn handle_ping(&self) -> String {
        let resp = SuccessResponse {
            success: true,
            data: PingData {
                status: "ok".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };
        serde_json::to_string(&resp).unwrap_or_else(|_| self.error_response("INTERNAL_ERROR", "Serialization failed"))
    }

    fn handle_print_batch(&self, msg: RawMessage) -> String {
        let pdf_urls = match msg.pdf_urls {
            Some(urls) => urls,
            None => return self.error_response("VALIDATION_ERROR", "Missing pdf_urls"),
        };
        let printer_name = match msg.printer_name {
            Some(name) => {
                let trimmed = name.trim().to_string();
                if trimmed.is_empty() {
                    return self.error_response("VALIDATION_ERROR", "printer_name must not be empty");
                }
                trimmed
            }
            _ => return self.error_response("VALIDATION_ERROR", "Missing printer_name"),
        };

        if pdf_urls.is_empty() {
            return self.error_response("VALIDATION_ERROR", "pdf_urls must not be empty");
        }
        if pdf_urls.len() > 5000 {
            return self.error_response("VALIDATION_ERROR", "pdf_urls exceeds maximum of 5000");
        }
        // Validate each URL has an http/https scheme.
        if let Some(bad_url) = pdf_urls.iter().find(|u| !Self::is_valid_print_url(u)) {
            return self.error_response("VALIDATION_ERROR", &format!("Invalid URL scheme: {}", bad_url));
        }

        let use_case = CreatePrintJobUseCase {
            job_repo: self.job_repo.clone(),
            event_store: self.event_store.clone(),
            event_bus: self.event_bus.clone(),
        };

        let request = CreateJobRequest {
            pdf_urls,
            printer_name,
            output_path: None,
        };

        match use_case.execute(request) {
            Ok(job_ids) => {
                let ids: Vec<String> = job_ids.iter().map(|id| id.to_string()).collect();
                let resp = SuccessResponse {
                    success: true,
                    data: PrintBatchData { job_ids: ids },
                };
                serde_json::to_string(&resp)
                    .unwrap_or_else(|_| self.error_response("INTERNAL_ERROR", "Serialization failed"))
            }
            Err(e) => self.application_error_response(&e),
        }
    }

    fn handle_get_status(&self, msg: RawMessage) -> String {
        let job_id_str = match msg.job_id {
            Some(id) if !id.is_empty() => id,
            _ => return self.error_response("VALIDATION_ERROR", "Missing or empty job_id"),
        };

        let use_case = GetJobStatusUseCase::new(self.job_repo.clone());

        match use_case.execute(&job_id_str) {
            Ok(dto) => {
                let resp = SuccessResponse {
                    success: true,
                    data: dto,
                };
                serde_json::to_string(&resp)
                    .unwrap_or_else(|_| self.error_response("INTERNAL_ERROR", "Serialization failed"))
            }
            Err(e) => self.application_error_response(&e),
        }
    }

    fn handle_cancel_job(&self, msg: RawMessage) -> String {
        let job_id = match msg.job_id {
            Some(id) if !id.is_empty() => id,
            _ => return self.error_response("VALIDATION_ERROR", "Missing or empty job_id"),
        };

        let use_case = CancelPrintJobUseCase::new(
            self.job_repo.clone(),
            self.event_store.clone(),
            self.event_bus.clone(),
        );

        let request = CancelJobRequest { job_id };

        match use_case.execute(request) {
            Ok(()) => {
                let resp = SuccessResponse {
                    success: true,
                    data: CancelData { cancelled: true },
                };
                serde_json::to_string(&resp)
                    .unwrap_or_else(|_| self.error_response("INTERNAL_ERROR", "Serialization failed"))
            }
            Err(e) => self.application_error_response(&e),
        }
    }

    fn handle_list_printers(&self) -> String {
        let printers: Vec<PrinterDto> = vec![];

        let resp = SuccessResponse {
            success: true,
            data: ListPrintersData { printers },
        };
        serde_json::to_string(&resp)
            .unwrap_or_else(|_| self.error_response("INTERNAL_ERROR", "Serialization failed"))
    }

    fn is_valid_print_url(url: &str) -> bool {
        url.starts_with("http://") || url.starts_with("https://")
    }

    fn application_error_response(&self, err: &ApplicationError) -> String {
        let (code, message) = match err {
            ApplicationError::TooManyJobs { count } => (
                "VALIDATION_ERROR",
                format!("Too many URLs: {} (max 5000)", count),
            ),
            ApplicationError::EmptyJobList => (
                "VALIDATION_ERROR",
                "pdf_urls must not be empty".to_string(),
            ),
            ApplicationError::PrinterNotAvailable { name } => (
                "PRINTER_NOT_AVAILABLE",
                format!("Printer '{}' is not available or offline", name),
            ),
            ApplicationError::InvalidJobId { job_id } => (
                "VALIDATION_ERROR",
                format!("Invalid job ID: {}", job_id),
            ),
            ApplicationError::JobNotFound { job_id } => (
                "JOB_NOT_FOUND",
                format!("Job not found: {}", job_id),
            ),
            ApplicationError::CannotCancelCompleted { job_id } => (
                "INVALID_STATE",
                format!("Cannot cancel completed job: {}", job_id),
            ),
            ApplicationError::CannotCancelFailed { job_id } => (
                "INVALID_STATE",
                format!("Cannot cancel failed job: {}", job_id),
            ),
            ApplicationError::CannotCancelCancelled { job_id } => (
                "INVALID_STATE",
                format!("Cannot cancel already cancelled job: {}", job_id),
            ),
            ApplicationError::ValidationError { reason } => (
                "VALIDATION_ERROR",
                reason.clone(),
            ),
            ApplicationError::DomainError(e) => (
                "VALIDATION_ERROR",
                format!("Domain error: {}", e),
            ),
            ApplicationError::RepositoryError(reason) => (
                "INTERNAL_ERROR",
                format!("Repository error: {}", reason),
            ),
            ApplicationError::DomainRuleViolation { reason } => (
                "VALIDATION_ERROR",
                format!("Domain rule violation: {}", reason),
            ),
            ApplicationError::EventStoreError { reason } => (
                "INTERNAL_ERROR",
                format!("Event store error: {}", reason),
            ),
            ApplicationError::EventBusError { reason } => (
                "INTERNAL_ERROR",
                format!("Event bus error: {}", reason),
            ),
            ApplicationError::MetricsError { reason } => (
                "INTERNAL_ERROR",
                format!("Metrics error: {}", reason),
            ),
        };
        self.error_response(code, &message)
    }

    fn error_response(&self, code: &str, message: &str) -> String {
        let resp = ErrorResponse {
            success: false,
            error: ErrorBody {
                code: code.to_string(),
                message: message.to_string(),
            },
        };
        serde_json::to_string(&resp).unwrap_or_else(|_| {
            r#"{"success":false,"error":{"code":"INTERNAL_ERROR","message":"Serialization failed"}}"#.to_string()
        })
    }
}

// â”€â”€ Unit Tests â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€


