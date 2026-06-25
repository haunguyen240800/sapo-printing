pub mod protocol;
pub mod registry;

use std::io::{self, BufReader, BufWriter, Write};
use std::sync::Arc;

use crate::domain::print_job::repository::PrintJobRepository;
use crate::domain::printer::repository::PrinterRepository;
use crate::infrastructure::database::SqliteEventStore;
use crate::infrastructure::metrics::MetricsCollector;
use crate::infrastructure::printer::PrinterManager;
use crate::shared::event_bus::EventBus;

use self::protocol::{NativeMessageHandler, ProtocolError};

pub fn run_native_messaging(
    job_repo: Arc<dyn PrintJobRepository>,
    printer_repo: Arc<dyn PrinterRepository>,
    printer_manager: Arc<dyn PrinterManager>,
    event_store: Arc<SqliteEventStore>,
    event_bus: Arc<dyn EventBus>,
    metrics_collector: Arc<MetricsCollector>,
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
        metrics_collector,
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
            Err(ProtocolError::Eof) => {
                // Clean disconnect: flush remaining buffered output.
                let _ = writer.flush();
                break;
            }
            Err(ProtocolError::MessageTooLarge { size }) => {
                let err = r#"{"success":false,"error":{"code":"MESSAGE_TOO_LARGE","message":"Message exceeds 1MB limit"}}"#;
                let _ = protocol::write_message(&mut writer, err);
                log_to_file(&format!("Message too large: {} bytes", size));
                // Continue processing subsequent messages instead of disconnecting.
            }
            Err(ProtocolError::PartialMessage { expected, received }) => {
                // Body truncated — send error response before disconnecting.
                let err = r#"{"success":false,"error":{"code":"INVALID_REQUEST","message":"Truncated message body"}}"#;
                let _ = protocol::write_message(&mut writer, err);
                let _ = writer.flush();
                log_to_file(&format!("Partial message: expected {} bytes, received {}", expected, received));
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

    // Rotate log file if it exceeds 5 MB to prevent unbounded growth.
    if let Ok(metadata) = std::fs::metadata(&log_path) {
        if metadata.len() > 5 * 1024 * 1024 {
            let _ = std::fs::rename(&log_path, log_path.with_extension("log.old"));
        }
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // ISO-8601 format for easy correlation with other logs.
    let iso_timestamp = {
        let secs = timestamp as i64;
        let days = secs / 86400;
        let time_of_day = secs - days * 86400;
        let hours = time_of_day / 3600;
        let minutes = (time_of_day % 3600) / 60;
        let seconds = time_of_day % 60;
        // Days since UNIX epoch (1970-01-01)
        let (year, month, day) = days_to_ymd(days);
        format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", year, month, day, hours, minutes, seconds)
    };

    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .and_then(|mut f| {
            use std::io::Write;
            writeln!(f, "[{}] {}", iso_timestamp, message)
        });
}

/// Convert days since UNIX epoch to (year, month, day). Simple algorithm
/// sufficient for log timestamp formatting (no timezone needed — UTC).
fn days_to_ymd(days: i64) -> (i64, u32, u32) {
    // Algorithm from Howard Hinnant's civil calendar.
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let yr = if m <= 2 { y + 1 } else { y };
    (yr, m as u32, d as u32)
}
