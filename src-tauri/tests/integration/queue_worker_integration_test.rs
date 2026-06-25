//! Queue Worker Integration Test
//!
//! Tests the complete job flow from QUEUED → COMPLETED using real infrastructure:
//! - SQLite database (in-memory)
//! - Real QueueManager, PrintJobRepository, EventStore
//! - Mock downloader, renderer, printer engine

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use rusqlite::Connection;
use sapo_printer::domain::print_job::aggregate::PrintJob;
use sapo_printer::domain::print_job::value_objects::{JobId, PrintStatus};
use sapo_printer::domain::print_job::PrintJobRepository;
use sapo_printer::infrastructure::database::{
    run_migrations, SqlitePrintJobRepository,
};
use sapo_printer::infrastructure::downloader::DocumentDownloader;
use sapo_printer::infrastructure::printer::PrinterEngine;
use sapo_printer::infrastructure::queue::{QueueManager, QueueWorker, SqliteQueueManager};
use sapo_printer::infrastructure::renderer::{DocumentRenderer, RenderConfig};
use sapo_printer::shared::errors::InfrastructureError;
use sapo_printer::shared::event_bus::InMemoryEventBus;

use super::common;

// --- Mock Infrastructure ---

struct MockDownloader;

impl DocumentDownloader for MockDownloader {
    fn download(
        &self,
        _url: &str,
        _job_id: &JobId,
    ) -> Result<std::path::PathBuf, InfrastructureError> {
        Ok(std::path::PathBuf::from("/tmp/mock.pdf"))
    }
}

struct MockRenderer;

impl DocumentRenderer for MockRenderer {
    fn render(
        &self,
        _path: &std::path::Path,
        _config: &RenderConfig,
    ) -> Result<Vec<u8>, InfrastructureError> {
        Ok(vec![0x25, 0x50, 0x44, 0x46]) // Mock PDF data
    }
}

struct MockPrinterEngine;

impl PrinterEngine for MockPrinterEngine {
    fn print(&self, _printer_name: &str, _data: &[u8]) -> Result<(), InfrastructureError> {
        Ok(())
    }
}

#[test]
fn test_worker_processes_real_job_flow() {
    // Setup: in-memory DB + real dependencies
    let mut conn = Connection::open_in_memory().unwrap();
    run_migrations(&mut conn).unwrap();
    let arc_conn = Arc::new(Mutex::new(conn));

    let job_repo = Arc::new(SqlitePrintJobRepository::new(arc_conn.clone()));
    let queue_manager = Arc::new(SqliteQueueManager::new(arc_conn.clone()));
    let event_store = common::create_test_event_store(arc_conn.clone());
    let event_bus = Arc::new(InMemoryEventBus::new());

    // Mock infrastructure
    let downloader = Arc::new(MockDownloader);
    let renderer = Arc::new(MockRenderer);
    let printer_engine = Arc::new(MockPrinterEngine);

    // Create and save job
    let job = PrintJob::new(
        "https://s3.example.com/doc.pdf".to_string(),
        "HP".to_string(),
    );
    let job_id = job.id().clone();
    job_repo.save(&job).unwrap();
    queue_manager.push(&job_id).unwrap();

    // Start worker
    let worker = QueueWorker::new(
        queue_manager,
        job_repo.clone(),
        event_store,
        event_bus,
        downloader,
        renderer,
        printer_engine,
    );
    worker.start().unwrap();

    // Wait for processing (max 5s)
    let mut final_status = None;
    for _ in 0..10 {
        thread::sleep(Duration::from_millis(500));
        if let Ok(Some(loaded_job)) = job_repo.find_by_id(&job_id) {
            if loaded_job.status() == &PrintStatus::Completed {
                final_status = Some(PrintStatus::Completed);
                break;
            }
        }
    }

    // Stop worker
    worker.stop().unwrap();

    // Verify job completed
    assert_eq!(
        final_status,
        Some(PrintStatus::Completed),
        "Job should reach Completed status"
    );

    let final_job = job_repo.find_by_id(&job_id).unwrap().unwrap();
    assert_eq!(final_job.status(), &PrintStatus::Completed);
}

#[test]
fn test_worker_processes_multiple_jobs_sequentially() {
    // Setup
    let mut conn = Connection::open_in_memory().unwrap();
    run_migrations(&mut conn).unwrap();
    let arc_conn = Arc::new(Mutex::new(conn));

    let job_repo = Arc::new(SqlitePrintJobRepository::new(arc_conn.clone()));
    let queue_manager = Arc::new(SqliteQueueManager::new(arc_conn.clone()));
    let event_store = common::create_test_event_store(arc_conn.clone());
    let event_bus = Arc::new(InMemoryEventBus::new());

    let downloader = Arc::new(MockDownloader);
    let renderer = Arc::new(MockRenderer);
    let printer_engine = Arc::new(MockPrinterEngine);

    // Create and queue 3 jobs
    let mut job_ids = Vec::new();
    for i in 1..=3 {
        let job = PrintJob::new(
            format!("https://s3.example.com/doc{}.pdf", i),
            "HP".to_string(),
        );
        let job_id = job.id().clone();
        job_ids.push(job_id.clone());
        job_repo.save(&job).unwrap();
        queue_manager.push(&job_id).unwrap();
    }

    // Start worker
    let worker = QueueWorker::new(
        queue_manager,
        job_repo.clone(),
        event_store,
        event_bus,
        downloader,
        renderer,
        printer_engine,
    );
    worker.start().unwrap();

    // Wait for all jobs to complete (max 10s)
    let mut all_completed = false;
    for _ in 0..20 {
        thread::sleep(Duration::from_millis(500));
        let completed_count = job_ids
            .iter()
            .filter(|id| {
                job_repo
                    .find_by_id(id)
                    .ok()
                    .flatten()
                    .map(|j| j.status() == &PrintStatus::Completed)
                    .unwrap_or(false)
            })
            .count();

        if completed_count == 3 {
            all_completed = true;
            break;
        }
    }

    worker.stop().unwrap();

    // Verify all jobs completed
    assert!(all_completed, "All 3 jobs should complete");
    for job_id in &job_ids {
        let job = job_repo.find_by_id(job_id).unwrap().unwrap();
        assert_eq!(job.status(), &PrintStatus::Completed);
    }
}
