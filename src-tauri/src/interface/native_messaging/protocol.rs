use std::io::{self, Read, Write};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::application::dto::create_job_request::CreateJobRequest;
use crate::application::dto::cancel_job_request::CancelJobRequest;
use crate::application::dto::job_dto::JobDto;
use crate::application::use_cases::cancel_print_job::CancelPrintJobUseCase;
use crate::application::use_cases::create_print_job::CreatePrintJobUseCase;
use crate::application::use_cases::errors::ApplicationError;
use crate::domain::print_job::repository::PrintJobRepository;
use crate::domain::printer::repository::PrinterRepository;
use crate::infrastructure::database::SqliteEventStore;
use crate::infrastructure::printer::PrinterManager;
use crate::shared::event_bus::EventBus;

const MAX_MESSAGE_SIZE: u32 = 1_048_576; // 1 MB

// ── Wire Protocol ───────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum ProtocolError {
    Io(io::Error),
    MessageTooLarge { size: u32 },
    InvalidUtf8,
    Eof,
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolError::Io(e) => write!(f, "IO error: {}", e),
            ProtocolError::MessageTooLarge { size } => write!(f, "Message too large: {} bytes", size),
            ProtocolError::InvalidUtf8 => write!(f, "Invalid UTF-8 in message"),
            ProtocolError::Eof => write!(f, "End of input"),
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
    reader.read_exact(&mut len_buf)?;
    let msg_len = u32::from_le_bytes(len_buf);

    if msg_len > MAX_MESSAGE_SIZE {
        return Err(ProtocolError::MessageTooLarge { size: msg_len });
    }

    let mut buf = vec![0u8; msg_len as usize];
    reader.read_exact(&mut buf)?;

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

// ── Origin Validation ───────────────────────────────────────────────────────

const ALLOWED_ORIGINS: &[&str] = &[];

pub fn parse_origin() -> Option<String> {
    std::env::args().find(|arg| arg.starts_with("chrome-extension://"))
}

pub fn validate_origin(origin: &Option<String>) -> bool {
    if ALLOWED_ORIGINS.is_empty() {
        return true;
    }
    match origin {
        Some(o) => ALLOWED_ORIGINS.iter().any(|allowed| o == allowed),
        None => false,
    }
}

// ── JSON Command Types ──────────────────────────────────────────────────────

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
    printers: Vec<PrinterInfo>,
}

#[derive(Serialize)]
struct PrinterInfo {
    name: String,
    device_id: String,
    status: String,
    printer_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_default: Option<bool>,
}

// ── NativeMessageHandler ────────────────────────────────────────────────────

pub struct NativeMessageHandler {
    pub job_repo: Arc<dyn PrintJobRepository>,
    pub printer_repo: Arc<dyn PrinterRepository>,
    pub printer_manager: Arc<dyn PrinterManager>,
    pub event_store: Arc<SqliteEventStore>,
    pub event_bus: Arc<dyn EventBus>,
}

impl NativeMessageHandler {
    pub fn new(
        job_repo: Arc<dyn PrintJobRepository>,
        printer_repo: Arc<dyn PrinterRepository>,
        printer_manager: Arc<dyn PrinterManager>,
        event_store: Arc<SqliteEventStore>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            job_repo,
            printer_repo,
            printer_manager,
            event_store,
            event_bus,
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

        match command.as_str() {
            "ping" => self.handle_ping(),
            "print_batch" => self.handle_print_batch(msg),
            "get_status" => self.handle_get_status(msg),
            "cancel_job" => self.handle_cancel_job(msg),
            "list_printers" => self.handle_list_printers(),
            unknown => self.error_response("UNKNOWN_COMMAND", &format!("Unknown command: {}", unknown)),
        }
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
            Some(name) if !name.is_empty() => name,
            _ => return self.error_response("VALIDATION_ERROR", "Missing or empty printer_name"),
        };

        if pdf_urls.is_empty() {
            return self.error_response("VALIDATION_ERROR", "pdf_urls must not be empty");
        }
        if pdf_urls.len() > 5000 {
            return self.error_response("VALIDATION_ERROR", "pdf_urls exceeds maximum of 5000");
        }

        let use_case = CreatePrintJobUseCase {
            job_repo: self.job_repo.clone(),
            event_store: self.event_store.clone(),
            event_bus: self.event_bus.clone(),
            printer_repo: self.printer_repo.clone(),
        };

        let request = CreateJobRequest {
            pdf_urls,
            printer_name,
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

        let job_id = match job_id_str.parse() {
            Ok(id) => id,
            Err(_) => return self.error_response("VALIDATION_ERROR", "Invalid job_id format"),
        };

        let job = match self.job_repo.find_by_id(&job_id) {
            Ok(Some(job)) => job,
            Ok(None) => return self.error_response("JOB_NOT_FOUND", &format!("Job not found: {}", job_id_str)),
            Err(e) => return self.error_response("INTERNAL_ERROR", &format!("Repository error: {}", e)),
        };

        let dto = JobDto::from(job);
        let resp = SuccessResponse {
            success: true,
            data: dto,
        };
        serde_json::to_string(&resp)
            .unwrap_or_else(|_| self.error_response("INTERNAL_ERROR", "Serialization failed"))
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
        let discovered = self.printer_manager.discover_printers();

        let printers: Vec<PrinterInfo> = discovered
            .iter()
            .map(|p| PrinterInfo {
                name: p.name().as_str().to_string(),
                device_id: p.name().as_str().to_string(),
                status: match p.status() {
                    crate::domain::printer::PrinterStatus::Online => "Online".to_string(),
                    crate::domain::printer::PrinterStatus::Offline => "Offline".to_string(),
                    crate::domain::printer::PrinterStatus::Error => "Error".to_string(),
                },
                printer_type: match p.printer_type() {
                    crate::domain::printer::PrinterType::Local => "Local".to_string(),
                    crate::domain::printer::PrinterType::Network => "Network".to_string(),
                },
                is_default: None,
            })
            .collect();

        let resp = SuccessResponse {
            success: true,
            data: ListPrintersData { printers },
        };
        serde_json::to_string(&resp)
            .unwrap_or_else(|_| self.error_response("INTERNAL_ERROR", "Serialization failed"))
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
            _ => (
                "INTERNAL_ERROR",
                format!("{}", err),
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

// ── Unit Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::print_job::aggregate::PrintJob;
    use crate::domain::print_job::errors::DomainError as JobDomainError;
    use crate::domain::print_job::repository::PrintJobRepository;
    use crate::domain::print_job::value_objects::{JobId, PrintStatus};
    use crate::domain::printer::aggregate::Printer;
    use crate::domain::printer::errors::PrinterDomainError;
    use crate::domain::printer::repository::PrinterRepository;
    use crate::domain::printer::value_objects::{PrinterName, PrinterType};
    use crate::infrastructure::database::{run_migrations, SqliteEventStore};
    use crate::infrastructure::printer::PrinterManager;
    use crate::shared::event_bus::InMemoryEventBus;
    use std::sync::Mutex as StdMutex;

    // ── Wire Protocol Tests ─────────────────────────────────────────────

    #[test]
    fn test_read_message_correctly_parses_framing() {
        let payload = r#"{"command":"ping"}"#;
        let len = (payload.len() as u32).to_le_bytes();
        let mut data: Vec<u8> = len.to_vec();
        data.extend_from_slice(payload.as_bytes());

        let mut cursor = io::Cursor::new(data);
        let msg = read_message(&mut cursor).unwrap();
        assert_eq!(msg, payload);
    }

    #[test]
    fn test_write_message_correctly_serializes_framing() {
        let payload = r#"{"success":true}"#;
        let mut buf = Vec::new();
        write_message(&mut buf, payload).unwrap();

        let expected_len = (payload.len() as u32).to_le_bytes();
        assert_eq!(&buf[0..4], &expected_len);
        assert_eq!(&buf[4..], payload.as_bytes());
    }

    #[test]
    fn test_read_message_exceeding_1mb_returns_error() {
        let fake_len = (MAX_MESSAGE_SIZE + 1).to_le_bytes();
        let mut data = fake_len.to_vec();
        data.extend(vec![0u8; 100]); // some dummy bytes

        let mut cursor = io::Cursor::new(data);
        let result = read_message(&mut cursor);
        assert!(matches!(result, Err(ProtocolError::MessageTooLarge { .. })));
    }

    #[test]
    fn test_read_message_eof_returns_error() {
        let data: Vec<u8> = vec![];
        let mut cursor = io::Cursor::new(data);
        let result = read_message(&mut cursor);
        assert!(matches!(result, Err(ProtocolError::Eof)));
    }

    #[test]
    fn test_write_read_roundtrip() {
        let original = r#"{"command":"ping"}"#;
        let mut buf = Vec::new();
        write_message(&mut buf, original).unwrap();

        let mut cursor = io::Cursor::new(buf);
        let decoded = read_message(&mut cursor).unwrap();
        assert_eq!(decoded, original);
    }

    // ── Origin Validation Tests ─────────────────────────────────────────

    #[test]
    fn test_empty_allowed_origins_accepts_any() {
        assert!(validate_origin(&None));
        assert!(validate_origin(&Some("chrome-extension://abc123/".to_string())));
    }

    // ── Handler Tests ───────────────────────────────────────────────────

    struct MockJobRepo {
        jobs: StdMutex<Vec<PrintJob>>,
    }

    impl MockJobRepo {
        fn new() -> Self {
            Self { jobs: StdMutex::new(Vec::new()) }
        }

        fn add_job(&self, job: PrintJob) {
            self.jobs.lock().unwrap().push(job);
        }
    }

    impl PrintJobRepository for MockJobRepo {
        fn save(&self, job: &PrintJob) -> Result<(), JobDomainError> {
            self.jobs.lock().unwrap().push(job.clone());
            Ok(())
        }
        fn update(&self, job: &PrintJob) -> Result<(), JobDomainError> {
            let mut jobs = self.jobs.lock().unwrap();
            if let Some(pos) = jobs.iter().position(|j| j.id() == job.id()) {
                jobs[pos] = job.clone();
                Ok(())
            } else {
                Err(JobDomainError::RepositoryError { reason: "not found".into() })
            }
        }
        fn find_by_id(&self, id: &JobId) -> Result<Option<PrintJob>, JobDomainError> {
            Ok(self.jobs.lock().unwrap().iter().find(|j| j.id() == id).cloned())
        }
        fn find_by_status(&self, _status: &PrintStatus) -> Result<Vec<PrintJob>, JobDomainError> {
            Ok(self.jobs.lock().unwrap().clone())
        }
        fn find_all(&self) -> Result<Vec<PrintJob>, JobDomainError> {
            Ok(self.jobs.lock().unwrap().clone())
        }
    }

    struct MockPrinterRepo;

    impl PrinterRepository for MockPrinterRepo {
        fn save(&self, _printer: &Printer) -> Result<(), PrinterDomainError> {
            Ok(())
        }
        fn find_all(&self) -> Result<Vec<Printer>, PrinterDomainError> {
            Ok(vec![])
        }
        fn find_by_name(&self, name: &PrinterName) -> Result<Option<Printer>, PrinterDomainError> {
            let mut p = Printer::new(name.clone(), PrinterType::Local);
            p.connect()?;
            Ok(Some(p))
        }
    }

    struct MockPrinterManager;

    impl PrinterManager for MockPrinterManager {
        fn discover_printers(&self) -> Vec<Printer> {
            vec![Printer::new(PrinterName::new("TestPrinter".to_string()), PrinterType::Local)]
        }
        fn get_status(&self, _name: &str) -> crate::domain::printer::PrinterStatus {
            crate::domain::printer::PrinterStatus::Online
        }
        fn supports_direct_pdf(&self, _name: &str) -> bool {
            true
        }
    }

    fn setup_handler() -> NativeMessageHandler {
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();
        let arc_conn = Arc::new(StdMutex::new(conn));

        NativeMessageHandler {
            job_repo: Arc::new(MockJobRepo::new()),
            printer_repo: Arc::new(MockPrinterRepo),
            printer_manager: Arc::new(MockPrinterManager),
            event_store: Arc::new(SqliteEventStore::new(arc_conn)),
            event_bus: Arc::new(InMemoryEventBus::new()),
        }
    }

    #[test]
    fn test_ping_returns_ok() {
        let handler = setup_handler();
        let resp = handler.handle_message(r#"{"command":"ping"}"#);
        let parsed: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["data"]["status"], "ok");
        assert!(parsed["data"]["version"].is_string());
    }

    #[test]
    fn test_print_batch_with_empty_urls_returns_validation_error() {
        let handler = setup_handler();
        let resp = handler.handle_message(r#"{"command":"print_batch","pdf_urls":[],"printer_name":"HP"}"#);
        let parsed: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(parsed["success"], false);
        assert_eq!(parsed["error"]["code"], "VALIDATION_ERROR");
    }

    #[test]
    fn test_print_batch_with_too_many_urls_returns_validation_error() {
        let handler = setup_handler();
        let urls: Vec<String> = (0..5001).map(|i| format!("http://example.com/{}.pdf", i)).collect();
        let payload = serde_json::json!({
            "command": "print_batch",
            "pdf_urls": urls,
            "printer_name": "HP"
        });
        let resp = handler.handle_message(&payload.to_string());
        let parsed: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(parsed["success"], false);
        assert_eq!(parsed["error"]["code"], "VALIDATION_ERROR");
    }

    #[test]
    fn test_print_batch_with_valid_request_returns_job_ids() {
        let handler = setup_handler();
        let payload = r#"{"command":"print_batch","pdf_urls":["https://example.com/doc.pdf"],"printer_name":"TestPrinter"}"#;
        let resp = handler.handle_message(payload);
        let parsed: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(parsed["success"], true);
        assert!(parsed["data"]["job_ids"].is_array());
        assert_eq!(parsed["data"]["job_ids"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_get_status_with_invalid_job_id_returns_not_found() {
        let handler = setup_handler();
        let resp = handler.handle_message(r#"{"command":"get_status","job_id":"00000000-0000-0000-0000-000000000000"}"#);
        let parsed: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(parsed["success"], false);
        assert_eq!(parsed["error"]["code"], "JOB_NOT_FOUND");
    }

    #[test]
    fn test_get_status_with_valid_job_returns_dto() {
        let handler = setup_handler();
        let job = PrintJob::new("https://example.com/doc.pdf".to_string(), "HP".to_string());
        let job_id = job.id().to_string();
        handler.job_repo.save(&job).unwrap();

        let payload = serde_json::json!({
            "command": "get_status",
            "job_id": job_id
        });
        let resp = handler.handle_message(&payload.to_string());
        let parsed: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["data"]["job_id"], job_id);
        assert_eq!(parsed["data"]["status"], "PENDING");
    }

    #[test]
    fn test_cancel_job_with_completed_job_returns_invalid_state() {
        let handler = setup_handler();
        let mut job = PrintJob::new("https://example.com/doc.pdf".to_string(), "HP".to_string());
        job.queue().unwrap();
        job.mark_downloaded().unwrap();
        job.mark_submitted().unwrap();
        job.mark_printing().unwrap();
        job.complete().unwrap();
        let job_id = job.id().to_string();
        handler.job_repo.save(&job).unwrap();

        let payload = serde_json::json!({
            "command": "cancel_job",
            "job_id": job_id
        });
        let resp = handler.handle_message(&payload.to_string());
        let parsed: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(parsed["success"], false);
        assert_eq!(parsed["error"]["code"], "INVALID_STATE");
    }

    #[test]
    fn test_cancel_job_with_pending_job_succeeds() {
        let handler = setup_handler();
        let job = PrintJob::new("https://example.com/doc.pdf".to_string(), "HP".to_string());
        let job_id = job.id().to_string();
        handler.job_repo.save(&job).unwrap();

        let payload = serde_json::json!({
            "command": "cancel_job",
            "job_id": job_id
        });
        let resp = handler.handle_message(&payload.to_string());
        let parsed: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["data"]["cancelled"], true);
    }

    #[test]
    fn test_list_printers_returns_discovered() {
        let handler = setup_handler();
        let resp = handler.handle_message(r#"{"command":"list_printers"}"#);
        let parsed: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(parsed["success"], true);
        let printers = parsed["data"]["printers"].as_array().unwrap();
        assert_eq!(printers.len(), 1);
        assert_eq!(printers[0]["name"], "TestPrinter");
    }

    #[test]
    fn test_invalid_json_returns_invalid_request() {
        let handler = setup_handler();
        let resp = handler.handle_message("not json at all");
        let parsed: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(parsed["success"], false);
        assert_eq!(parsed["error"]["code"], "INVALID_REQUEST");
    }

    #[test]
    fn test_missing_command_returns_invalid_request() {
        let handler = setup_handler();
        let resp = handler.handle_message(r#"{"pdf_urls":[]}"#);
        let parsed: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(parsed["success"], false);
        assert_eq!(parsed["error"]["code"], "INVALID_REQUEST");
    }

    #[test]
    fn test_unknown_command_returns_unknown_command() {
        let handler = setup_handler();
        let resp = handler.handle_message(r#"{"command":"fly_to_moon"}"#);
        let parsed: serde_json::Value = serde_json::from_str(&resp).unwrap();
        assert_eq!(parsed["success"], false);
        assert_eq!(parsed["error"]["code"], "UNKNOWN_COMMAND");
    }
}
