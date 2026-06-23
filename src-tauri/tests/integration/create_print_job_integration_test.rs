//! Integration tests for CreatePrintJobUseCase.
//!
//! Uses real in-memory SQLite to verify:
//! 1. Job created with PENDING status → find_by_id returns it
//! 2. Event stored → find_by_aggregate returns PrintJobCreated
//! 3. Offline printer → error, nothing saved

use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use sapo_printer::application::dto::create_job_request::CreateJobRequest;
use sapo_printer::application::use_cases::create_print_job::CreatePrintJobUseCase;
use sapo_printer::domain::print_job::repository::PrintJobRepository;
use sapo_printer::domain::print_job::value_objects::PrintStatus;
use sapo_printer::domain::printer::aggregate::Printer;
use sapo_printer::domain::printer::repository::PrinterRepository;
use sapo_printer::domain::printer::value_objects::{PrinterName, PrinterType};
use sapo_printer::infrastructure::database::migrations::run_migrations;
use sapo_printer::infrastructure::database::{
    SqliteEventStore, SqlitePrintJobRepository, SqlitePrinterRepository,
};
use sapo_printer::shared::event_bus::InMemoryEventBus;

fn setup_test_deps() -> (
    Arc<SqlitePrintJobRepository>,
    Arc<SqliteEventStore>,
    Arc<InMemoryEventBus>,
    Arc<SqlitePrinterRepository>,
    Arc<Mutex<Connection>>,
) {
    let mut conn = Connection::open_in_memory().unwrap();
    run_migrations(&mut conn).unwrap();
    let arc_conn = Arc::new(Mutex::new(conn));

    let job_repo = Arc::new(SqlitePrintJobRepository::new(arc_conn.clone()));
    let event_store = Arc::new(SqliteEventStore::new(arc_conn.clone()));
    let event_bus = Arc::new(InMemoryEventBus::new());
    let printer_repo = Arc::new(SqlitePrinterRepository::new(arc_conn.clone()));

    (job_repo, event_store, event_bus, printer_repo, arc_conn)
}

fn seed_online_printer(
    printer_repo: &Arc<SqlitePrinterRepository>,
    conn: &Arc<Mutex<Connection>>,
    name: &str,
) {
    // Save printer via repository (starts Offline)
    let printer = Printer::new(PrinterName::new(name.to_string()), PrinterType::Local);
    printer_repo.save(&printer).unwrap();

    // Manually set status to Online via direct SQL
    let c = conn.lock().unwrap_or_else(|p| p.into_inner());
    c.execute(
        "UPDATE printer_configs SET status = 'Online' WHERE printer_name = ?1",
        [name],
    )
    .unwrap();
}

#[test]
fn test_integration_create_job_persists_with_pending_status() {
    let (job_repo, event_store, event_bus, printer_repo, conn) = setup_test_deps();
    seed_online_printer(&printer_repo, &conn, "HP_Integration1");

    let use_case = CreatePrintJobUseCase {
        job_repo: job_repo.clone(),
        event_store,
        event_bus,
        printer_repo,
    };

    let request = CreateJobRequest {
        pdf_urls: vec!["https://s3.example.com/integration.pdf".to_string()],
        printer_name: "HP_Integration1".to_string(),
    };
    let result = use_case.execute(request);
    assert!(result.is_ok());
    let ids = result.unwrap();
    assert_eq!(ids.len(), 1);

    // Verify via repository: find_by_id returns job with PENDING status
    let found = job_repo.find_by_id(&ids[0]).unwrap();
    assert!(found.is_some());
    let job = found.unwrap();
    assert_eq!(*job.status(), PrintStatus::Pending);
    assert_eq!(job.pdf_url(), "https://s3.example.com/integration.pdf");
}

#[test]
fn test_integration_event_stored_in_event_store() {
    let (job_repo, event_store, event_bus, printer_repo, conn) = setup_test_deps();
    seed_online_printer(&printer_repo, &conn, "HP_Integration2");

    let use_case = CreatePrintJobUseCase {
        job_repo,
        event_store: event_store.clone(),
        event_bus,
        printer_repo,
    };

    let request = CreateJobRequest {
        pdf_urls: vec!["https://s3.example.com/event-test.pdf".to_string()],
        printer_name: "HP_Integration2".to_string(),
    };
    let result = use_case.execute(request);
    assert!(result.is_ok());
    let ids = result.unwrap();

    // Verify event stored: find_by_aggregate returns PrintJobCreated
    let events = event_store.find_by_aggregate(&ids[0].to_string()).unwrap();
    assert!(!events.is_empty());
    assert_eq!(events[0].event_type, "PrintJobCreated");
}

#[test]
fn test_integration_offline_printer_returns_error_nothing_saved() {
    let (job_repo, event_store, event_bus, printer_repo, conn) = setup_test_deps();

    // Save printer with Offline status (don't update to Online)
    let printer = Printer::new(
        PrinterName::new("HP_Offline_Integration".to_string()),
        PrinterType::Local,
    );
    printer_repo.save(&printer).unwrap();

    let use_case = CreatePrintJobUseCase {
        job_repo,
        event_store,
        event_bus,
        printer_repo,
    };

    let request = CreateJobRequest {
        pdf_urls: vec!["https://s3.example.com/should-fail.pdf".to_string()],
        printer_name: "HP_Offline_Integration".to_string(),
    };
    let result = use_case.execute(request);
    assert!(result.is_err());

    // Verify nothing was saved
    let c = conn.lock().unwrap_or_else(|p| p.into_inner());
    let count: i64 = c
        .query_row("SELECT COUNT(*) FROM print_jobs", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);
}
