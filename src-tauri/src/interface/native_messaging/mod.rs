pub mod protocol;
pub mod registry;

use std::io::{self, BufReader, BufWriter};
use std::sync::Arc;

use crate::domain::print_job::repository::PrintJobRepository;
use crate::domain::printer::repository::PrinterRepository;
use crate::infrastructure::database::SqliteEventStore;
use crate::infrastructure::printer::PrinterManager;
use crate::shared::event_bus::EventBus;

use self::protocol::{NativeMessageHandler, ProtocolError};

pub fn run_native_messaging(
    job_repo: Arc<dyn PrintJobRepository>,
    printer_repo: Arc<dyn PrinterRepository>,
    printer_manager: Arc<dyn PrinterManager>,
    event_store: Arc<SqliteEventStore>,
    event_bus: Arc<dyn EventBus>,
) -> Result<(), String> {
    protocol::set_binary_mode();

    let origin = protocol::parse_origin();
    if !protocol::validate_origin(&origin) {
        let err = r#"{"success":false,"error":{"code":"UNAUTHORIZED_ORIGIN","message":"Origin not allowed"}}"#;
        let _ = protocol::write_message(&mut io::stdout(), err);
        return Err("Unauthorized origin".to_string());
    }

    let handler = NativeMessageHandler::new(
        job_repo,
        printer_repo,
        printer_manager,
        event_store,
        event_bus,
    );

    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = io::stdout();
    let mut writer = BufWriter::new(stdout.lock());

    loop {
        match protocol::read_message(&mut reader) {
            Ok(msg) => {
                let response = handler.handle_message(&msg);
                if let Err(e) = protocol::write_message(&mut writer, &response) {
                    log_to_file(&format!("Write error: {}", e));
                    break;
                }
            }
            Err(ProtocolError::Eof) => break,
            Err(ProtocolError::MessageTooLarge { size }) => {
                let err = r#"{"success":false,"error":{"code":"MESSAGE_TOO_LARGE","message":"Message exceeds 1MB limit"}}"#;
                let _ = protocol::write_message(&mut writer, err);
                log_to_file(&format!("Message too large: {} bytes", size));
                break;
            }
            Err(e) => {
                log_to_file(&format!("Read error: {}", e));
                break;
            }
        }
    }

    Ok(())
}

fn log_to_file(message: &str) {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let log_path = std::path::PathBuf::from(&home)
        .join(".sapo-printer")
        .join("native-messaging.log");
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .and_then(|mut f| {
            use std::io::Write;
            writeln!(f, "[{}] {}", timestamp, message)
        });
}
