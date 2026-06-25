//! Integration tests for PrintJobRepository and EventStore.
//!
//! Verifies full lifecycle: create → save → update → find,
//! event batch persistence, and job persistence across "restart".

use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use sapo_printer::domain::print_job::aggregate::PrintJob;
use sapo_printer::domain::print_job::repository::PrintJobRepository;
use sapo_printer::domain::print_job::value_objects::PrintStatus;
use sapo_printer::infrastructure::database::{
    run_migrations, DbPool, SqlitePrintJobRepository,
};

use super::common;

/// RAII guard that removes a temp file on drop.
struct TempDb {
    path: std::path::PathBuf,
}

impl TempDb {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(name);
        // Ensure no stale file from a previous run
        let _ = std::fs::remove_file(&path);
        Self { path }
    }

    fn path_str(&self) -> &str {
        self.path.to_str().expect("Non-UTF8 temp path")
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn setup_test_db() -> Arc<Mutex<Connection>> {
    let mut conn = Connection::open_in_memory().expect("Failed to open in-memory database");
    run_migrations(&mut conn).expect("Failed to run migrations");
    Arc::new(Mutex::new(conn))
}

#[test]
fn test_full_job_lifecycle() {
    let conn = setup_test_db();
    let repo = SqlitePrintJobRepository::new(conn);

    // Create and save
    let mut job = PrintJob::new(
        "https://s3.example.com/test.pdf".to_string(),
        "TestPrinter".to_string(),
    );
    let job_id = job.id().clone();
    repo.save(&job).unwrap();

    // Transition through states
    job.queue().unwrap();
    repo.update(&job).unwrap();

    // Find by ID
    let found = repo.find_by_id(&job_id).unwrap().expect("Job should exist");
    assert_eq!(*found.status(), PrintStatus::Queued);
    assert_eq!(found.printer_name(), "TestPrinter");

    // Find by status
    let queued = repo.find_by_status(&PrintStatus::Queued).unwrap();
    assert_eq!(queued.len(), 1);

    // Find all
    let all = repo.find_all().unwrap();
    assert_eq!(all.len(), 1);
}

#[test]
fn test_event_store_batch_and_sequence() {
    let conn = setup_test_db();
    let store = common::create_test_event_store(conn);

    // Create a job and accumulate events
    let mut job = PrintJob::new(
        "https://s3.example.com/doc.pdf".to_string(),
        "HP".to_string(),
    );
    let job_id = job.id().to_string();
    job.queue().unwrap();
    job.mark_downloaded().unwrap();

    let events = job.drain_events();
    let event_count = events.len();

    // Save all in batch
    store.save_all(&job_id, &events).unwrap();

    // Query by aggregate
    let stored = store.find_by_aggregate(&job_id).unwrap();
    assert_eq!(stored.len(), event_count);

    // Verify sequence ordering
    for (i, event) in stored.iter().enumerate() {
        assert_eq!(event.sequence_number, (i + 1) as i64);
    }
}

#[test]
fn test_job_persists_across_restart() {
    let temp_db = TempDb::new(&format!(
        "sapo-repo-integration-{}.db",
        uuid::Uuid::new_v4()
    ));

    let job_id;
    {
        // Phase 1: create and save
        let pool = DbPool::new(temp_db.path_str()).expect("Failed to create pool");
        run_migrations(&mut pool.get()).expect("Migrations failed");
        let repo = SqlitePrintJobRepository::new(pool.get_arc());

        let job = PrintJob::new(
            "https://s3.example.com/restart.pdf".to_string(),
            "RestartPrinter".to_string(),
        );
        job_id = job.id().clone();
        repo.save(&job).unwrap();

        drop(repo);
        drop(pool);
    }

    {
        // Phase 2: reopen and find
        let pool2 = DbPool::new(temp_db.path_str()).expect("Failed to reopen pool");
        run_migrations(&mut pool2.get()).expect("Migrations failed on reopen");
        let repo2 = SqlitePrintJobRepository::new(pool2.get_arc());

        let found = repo2
            .find_by_id(&job_id)
            .unwrap()
            .expect("Job should persist");
        assert_eq!(found.printer_name(), "RestartPrinter");
        assert_eq!(found.pdf_url(), "https://s3.example.com/restart.pdf");

        drop(repo2);
        drop(pool2);
    }
    // TempDb drop cleans up automatically
}
