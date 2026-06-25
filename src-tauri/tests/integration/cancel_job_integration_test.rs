use rusqlite::Connection;
use sapo_printer::application::dto::cancel_job_request::CancelJobRequest;
use sapo_printer::application::use_cases::cancel_print_job::CancelPrintJobUseCase;
use sapo_printer::domain::print_job::aggregate::PrintJob;
use sapo_printer::domain::print_job::repository::PrintJobRepository;
use sapo_printer::domain::print_job::value_objects::PrintStatus;
use sapo_printer::infrastructure::database::{
    run_migrations, SqlitePrintJobRepository,
};
use sapo_printer::shared::event_bus::InMemoryEventBus;
use std::sync::{Arc, Mutex as StdMutex};

use super::common;

#[test]
fn test_cancel_queued_job_end_to_end() {
    // Setup database
    let mut conn = Connection::open_in_memory().unwrap();
    run_migrations(&mut conn).unwrap();
    let arc_conn = Arc::new(StdMutex::new(conn));

    // Setup infrastructure
    let job_repo = Arc::new(SqlitePrintJobRepository::new(arc_conn.clone()));
    let event_store = common::create_test_event_store(arc_conn.clone());
    let event_bus = Arc::new(InMemoryEventBus::new());

    // Create and persist job
    let mut job = PrintJob::new(
        "https://s3.example.com/doc.pdf".to_string(),
        "HP Printer".to_string(),
    );
    job.queue().unwrap();
    let job_id = job.id().clone();
    job_repo.save(&job).unwrap();

    // Cancel job via use case
    let use_case = CancelPrintJobUseCase::new(job_repo.clone(), event_store.clone(), event_bus);
    let request = CancelJobRequest {
        job_id: job_id.to_string(),
    };
    let result = use_case.execute(request);

    assert!(result.is_ok());

    // Verify job persisted as CANCELLED
    let loaded_job = job_repo.find_by_id(&job_id).unwrap().unwrap();
    assert_eq!(*loaded_job.status(), PrintStatus::Cancelled);

    // Verify events persisted
    let events = event_store.find_by_aggregate(&job_id.to_string()).unwrap();
    assert!(events.iter().any(|e| e.event_type == "PrintJobCancelled"));
}

#[test]
fn test_cancel_downloaded_job_cleans_temp_file() {
    // Setup
    let mut conn = Connection::open_in_memory().unwrap();
    run_migrations(&mut conn).unwrap();
    let arc_conn = Arc::new(StdMutex::new(conn));

    let job_repo = Arc::new(SqlitePrintJobRepository::new(arc_conn.clone()));
    let event_store = common::create_test_event_store(arc_conn.clone());
    let event_bus = Arc::new(InMemoryEventBus::new());

    // Create job and simulate temp file
    let mut job = PrintJob::new(
        "https://s3.example.com/doc.pdf".to_string(),
        "HP".to_string(),
    );
    job.queue().unwrap();
    job.mark_downloaded().unwrap();
    let job_id = job.id().clone();
    job_repo.save(&job).unwrap();

    // Create temp file
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().to_string());
    let temp_dir = std::path::PathBuf::from(home).join(".sapo-printer/temp");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let temp_file = temp_dir.join(format!("{}.pdf", job_id));
    std::fs::write(&temp_file, b"fake pdf content").unwrap();
    assert!(temp_file.exists());

    // Cancel job
    let use_case = CancelPrintJobUseCase::new(job_repo, event_store, event_bus);
    use_case
        .execute(CancelJobRequest {
            job_id: job_id.to_string(),
        })
        .unwrap();

    // Verify temp file deleted
    assert!(!temp_file.exists());
}

#[test]
fn test_cannot_cancel_completed_job_integration() {
    let mut conn = Connection::open_in_memory().unwrap();
    run_migrations(&mut conn).unwrap();
    let arc_conn = Arc::new(StdMutex::new(conn));

    let job_repo = Arc::new(SqlitePrintJobRepository::new(arc_conn.clone()));
    let event_store = common::create_test_event_store(arc_conn.clone());
    let event_bus = Arc::new(InMemoryEventBus::new());

    // Create completed job
    let mut job = PrintJob::new(
        "https://s3.example.com/doc.pdf".to_string(),
        "HP".to_string(),
    );
    job.queue().unwrap();
    job.mark_downloaded().unwrap();
    job.mark_submitted().unwrap();
    job.mark_printing().unwrap();
    job.complete().unwrap();
    let job_id = job.id().clone();
    job_repo.save(&job).unwrap();

    // Attempt cancel
    let use_case = CancelPrintJobUseCase::new(job_repo.clone(), event_store, event_bus);
    let result = use_case.execute(CancelJobRequest {
        job_id: job_id.to_string(),
    });

    assert!(result.is_err());

    // Verify job still COMPLETED (not cancelled)
    let loaded_job = job_repo.find_by_id(&job_id).unwrap().unwrap();
    assert_eq!(*loaded_job.status(), PrintStatus::Completed);
}
