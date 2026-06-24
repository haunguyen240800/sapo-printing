//! Integration tests for Native Messaging protocol handler.
//!
//! Simulates full browser ↔ host communication using in-memory SQLite
//! and real repositories. Tests all 5 commands end-to-end plus error scenarios.

use std::io::Cursor;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use sapo_printer::domain::print_job::aggregate::PrintJob;
use sapo_printer::domain::print_job::repository::PrintJobRepository;
use sapo_printer::domain::print_job::value_objects::PrintStatus;
use sapo_printer::domain::printer::aggregate::Printer;
use sapo_printer::domain::printer::repository::PrinterRepository;
use sapo_printer::domain::printer::value_objects::{PrinterName, PrinterStatus, PrinterType};
use sapo_printer::infrastructure::database::migrations::run_migrations;
use sapo_printer::infrastructure::database::{
    SqliteEventStore, SqlitePrintJobRepository, SqlitePrinterRepository,
};
use sapo_printer::infrastructure::printer::PrinterManager;
use sapo_printer::interface::native_messaging::protocol::{
    read_message, write_message, NativeMessageHandler,
};
use sapo_printer::shared::event_bus::InMemoryEventBus;

struct NoopPrinterManager;

impl PrinterManager for NoopPrinterManager {
    fn discover_printers(&self) -> Vec<Printer> {
        vec![]
    }
    fn get_status(&self, _name: &str) -> PrinterStatus {
        PrinterStatus::Online
    }
    fn supports_direct_pdf(&self, _name: &str) -> bool {
        true
    }
}

struct OfflinePrinterManager;

impl PrinterManager for OfflinePrinterManager {
    fn discover_printers(&self) -> Vec<Printer> {
        vec![]
    }
    fn get_status(&self, _name: &str) -> PrinterStatus {
        PrinterStatus::Offline
    }
    fn supports_direct_pdf(&self, _name: &str) -> bool {
        false
    }
}

fn setup() -> (NativeMessageHandler, Arc<SqlitePrintJobRepository>, Arc<SqlitePrinterRepository>, Arc<Mutex<Connection>>) {
    let mut conn = Connection::open_in_memory().unwrap();
    run_migrations(&mut conn).unwrap();
    let arc_conn = Arc::new(Mutex::new(conn));

    let job_repo = Arc::new(SqlitePrintJobRepository::new(arc_conn.clone()));
    let printer_repo = Arc::new(SqlitePrinterRepository::new(arc_conn.clone()));
    let event_store = Arc::new(SqliteEventStore::new(arc_conn.clone()));
    let event_bus = Arc::new(InMemoryEventBus::new());
    let printer_manager: Arc<dyn PrinterManager> = Arc::new(NoopPrinterManager);

    let handler = NativeMessageHandler::new(
        job_repo.clone() as Arc<dyn PrintJobRepository>,
        printer_repo.clone() as Arc<dyn PrinterRepository>,
        printer_manager,
        event_store,
        event_bus as Arc<dyn sapo_printer::shared::event_bus::EventBus>,
    );

    (handler, job_repo, printer_repo, arc_conn)
}

fn seed_online_printer(
    printer_repo: &Arc<SqlitePrinterRepository>,
    conn: &Arc<Mutex<Connection>>,
    name: &str,
) {
    let printer = Printer::new(PrinterName::new(name.to_string()), PrinterType::Local);
    printer_repo.save(&printer).unwrap();
    let c = conn.lock().unwrap_or_else(|p| p.into_inner());
    c.execute(
        "UPDATE printer_configs SET status = 'Online' WHERE printer_name = ?1",
        [name],
    )
    .unwrap();
}

fn simulate_exchange(handler: &NativeMessageHandler, request: &str) -> serde_json::Value {
    let response = handler.handle_message(request);
    serde_json::from_str(&response).unwrap()
}

fn simulate_wire_exchange(handler: &NativeMessageHandler, request: &str) -> serde_json::Value {
    let mut buf = Vec::new();
    write_message(&mut buf, request).unwrap();

    let mut cursor = Cursor::new(buf);
    let decoded_request = read_message(&mut cursor).unwrap();
    let response = handler.handle_message(&decoded_request);

    let mut response_buf = Vec::new();
    write_message(&mut response_buf, &response).unwrap();

    let mut response_cursor = Cursor::new(response_buf);
    let decoded_response = read_message(&mut response_cursor).unwrap();
    serde_json::from_str(&decoded_response).unwrap()
}

#[test]
fn test_ping_end_to_end() {
    let (handler, _, _, _) = setup();
    let resp = simulate_exchange(&handler, r#"{"command":"ping"}"#);
    assert_eq!(resp["success"], true);
    assert_eq!(resp["data"]["status"], "ok");
}

#[test]
fn test_print_batch_end_to_end() {
    let (handler, _, printer_repo, conn) = setup();
    seed_online_printer(&printer_repo, &conn, "HP_Integration");

    let request = serde_json::json!({
        "command": "print_batch",
        "pdf_urls": ["https://example.com/doc1.pdf", "https://example.com/doc2.pdf"],
        "printer_name": "HP_Integration"
    });
    let resp = simulate_exchange(&handler, &request.to_string());
    assert_eq!(resp["success"], true);
    let job_ids = resp["data"]["job_ids"].as_array().unwrap();
    assert_eq!(job_ids.len(), 2);
}

#[test]
fn test_get_status_end_to_end() {
    let (handler, job_repo, printer_repo, conn) = setup();
    seed_online_printer(&printer_repo, &conn, "HP_Status");

    let job = PrintJob::new("https://example.com/doc.pdf".to_string(), "HP_Status".to_string());
    let job_id = job.id().to_string();
    job_repo.save(&job).unwrap();

    let request = serde_json::json!({
        "command": "get_status",
        "job_id": job_id
    });
    let resp = simulate_exchange(&handler, &request.to_string());
    assert_eq!(resp["success"], true);
    assert_eq!(resp["data"]["job_id"], job_id);
    assert_eq!(resp["data"]["status"], "PENDING");
    assert_eq!(resp["data"]["printer_name"], "HP_Status");
}

#[test]
fn test_cancel_job_end_to_end() {
    let (handler, job_repo, printer_repo, conn) = setup();
    seed_online_printer(&printer_repo, &conn, "HP_Cancel");

    let job = PrintJob::new("https://example.com/doc.pdf".to_string(), "HP_Cancel".to_string());
    let job_id = job.id().to_string();
    job_repo.save(&job).unwrap();

    let request = serde_json::json!({
        "command": "cancel_job",
        "job_id": job_id
    });
    let resp = simulate_exchange(&handler, &request.to_string());
    assert_eq!(resp["success"], true);
    assert_eq!(resp["data"]["cancelled"], true);

    let updated = job_repo.find_by_id(&job_id.parse().unwrap()).unwrap().unwrap();
    assert_eq!(*updated.status(), PrintStatus::Cancelled);
}

#[test]
fn test_list_printers_end_to_end() {
    let (handler, _, _, _) = setup();
    let resp = simulate_exchange(&handler, r#"{"command":"list_printers"}"#);
    assert_eq!(resp["success"], true);
    assert!(resp["data"]["printers"].is_array());
}

#[test]
fn test_malformed_json_returns_invalid_request() {
    let (handler, _, _, _) = setup();
    let resp = simulate_exchange(&handler, "this is not json");
    assert_eq!(resp["success"], false);
    assert_eq!(resp["error"]["code"], "INVALID_REQUEST");
}

#[test]
fn test_unknown_command_returns_unknown_command() {
    let (handler, _, _, _) = setup();
    let resp = simulate_exchange(&handler, r#"{"command":"teleport"}"#);
    assert_eq!(resp["success"], false);
    assert_eq!(resp["error"]["code"], "UNKNOWN_COMMAND");
}

#[test]
fn test_invalid_job_id_returns_not_found() {
    let (handler, _, _, _) = setup();
    let resp = simulate_exchange(
        &handler,
        r#"{"command":"get_status","job_id":"00000000-0000-0000-0000-000000000000"}"#,
    );
    assert_eq!(resp["success"], false);
    assert_eq!(resp["error"]["code"], "JOB_NOT_FOUND");
}

#[test]
fn test_wire_protocol_bidirectional() {
    let (handler, _, printer_repo, conn) = setup();
    seed_online_printer(&printer_repo, &conn, "HP_Wire");

    let resp = simulate_wire_exchange(&handler, r#"{"command":"ping"}"#);
    assert_eq!(resp["success"], true);
    assert_eq!(resp["data"]["status"], "ok");
}

#[test]
fn test_print_batch_empty_urls_via_wire() {
    let (handler, _, _, _) = setup();
    let request = serde_json::json!({
        "command": "print_batch",
        "pdf_urls": [],
        "printer_name": "HP"
    });
    let resp = simulate_wire_exchange(&handler, &request.to_string());
    assert_eq!(resp["success"], false);
    assert_eq!(resp["error"]["code"], "VALIDATION_ERROR");
}

#[test]
fn test_cancel_completed_job_returns_invalid_state() {
    let (handler, job_repo, printer_repo, conn) = setup();
    seed_online_printer(&printer_repo, &conn, "HP_Completed");

    let mut job = PrintJob::new("https://example.com/doc.pdf".to_string(), "HP_Completed".to_string());
    job.queue().unwrap();
    job.mark_downloaded().unwrap();
    job.mark_submitted().unwrap();
    job.mark_printing().unwrap();
    job.complete().unwrap();
    let job_id = job.id().to_string();
    job_repo.save(&job).unwrap();

    let request = serde_json::json!({
        "command": "cancel_job",
        "job_id": job_id
    });
    let resp = simulate_exchange(&handler, &request.to_string());
    assert_eq!(resp["success"], false);
    assert_eq!(resp["error"]["code"], "INVALID_STATE");
}

// AC-7 / M-8: print_batch with offline printer returns PRINTER_NOT_AVAILABLE
fn setup_with_offline_printer() -> (NativeMessageHandler, Arc<SqlitePrinterRepository>, Arc<Mutex<Connection>>) {
    let mut conn = Connection::open_in_memory().unwrap();
    run_migrations(&mut conn).unwrap();
    let arc_conn = Arc::new(Mutex::new(conn));

    let job_repo = Arc::new(SqlitePrintJobRepository::new(arc_conn.clone()));
    let printer_repo = Arc::new(SqlitePrinterRepository::new(arc_conn.clone()));
    let event_store = Arc::new(SqliteEventStore::new(arc_conn.clone()));
    let event_bus = Arc::new(InMemoryEventBus::new());
    let printer_manager: Arc<dyn PrinterManager> = Arc::new(OfflinePrinterManager);

    let handler = NativeMessageHandler::new(
        job_repo as Arc<dyn sapo_printer::domain::print_job::repository::PrintJobRepository>,
        printer_repo.clone() as Arc<dyn sapo_printer::domain::printer::repository::PrinterRepository>,
        printer_manager,
        event_store,
        event_bus as Arc<dyn sapo_printer::shared::event_bus::EventBus>,
    );

    (handler, printer_repo, arc_conn)
}

#[test]
fn test_print_batch_offline_printer_returns_not_available() {
    let (handler, printer_repo, conn) = setup_with_offline_printer();
    seed_online_printer(&printer_repo, &conn, "OfflinePrinter");

    let request = serde_json::json!({
        "command": "print_batch",
        "pdf_urls": ["https://example.com/doc.pdf"],
        "printer_name": "OfflinePrinter"
    });
    let resp = simulate_exchange(&handler, &request.to_string());
    assert_eq!(resp["success"], false);
    assert_eq!(resp["error"]["code"], "PRINTER_NOT_AVAILABLE");
}
