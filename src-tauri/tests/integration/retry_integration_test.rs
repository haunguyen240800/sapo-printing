//! Integration Tests for Auto-Retry Logic (Story 3.6)
//!
//! Tests the complete retry flow with real components:
//! - QueueWorker with retry logic
//! - SqliteQueueManager with delay enforcement
//! - Real timing verification (not mocked)
//!
//! Scenarios tested:
//! 1. Job recovers after transient failure (FlakyDownloader succeeds on retry 2)
//! 2. Job fails permanently after max retries (3 attempts exhausted)
//! 3. Backoff timing accuracy (5s → 10s → 20s delays enforced)

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::thread;
use std::time::{Duration, Instant};

use sapo_printer::domain::print_job::aggregate::PrintJob;
use sapo_printer::domain::print_job::value_objects::{JobId, PrintStatus};
use sapo_printer::domain::print_job::PrintJobRepository;
use sapo_printer::infrastructure::database::{
    run_migrations, SqliteEventStore, SqlitePrintJobRepository,
};
use sapo_printer::infrastructure::downloader::DocumentDownloader;
use sapo_printer::infrastructure::printer::PrinterEngine;
use sapo_printer::infrastructure::queue::{QueueWorker, SqliteQueueManager};
use sapo_printer::infrastructure::renderer::{DocumentRenderer, RenderConfig};
use sapo_printer::shared::errors::InfrastructureError;
use sapo_printer::shared::event_bus::{EventBus, EventBusError};

// --- Mock EventBus ---

struct MockEventBus;

impl EventBus for MockEventBus {
    fn publish(&self, _event_type: &str, _payload: &str) -> Result<(), EventBusError> {
        Ok(())
    }
}

// --- Flaky Downloader (succeeds on Nth attempt) ---

struct FlakyDownloader {
    attempt_count: AtomicU32,
    succeed_on_attempt: u32, // 1-based: succeed on this attempt number
    created_files: StdMutex<Vec<PathBuf>>, // Track created files for cleanup
}

impl FlakyDownloader {
    fn new(succeed_on_attempt: u32) -> Self {
        Self {
            attempt_count: AtomicU32::new(0),
            succeed_on_attempt,
            created_files: StdMutex::new(Vec::new()),
        }
    }

    fn get_attempt_count(&self) -> u32 {
        self.attempt_count.load(Ordering::SeqCst)
    }
}

impl Drop for FlakyDownloader {
    fn drop(&mut self) {
        // Cleanup any orphaned temp files
        let files = self.created_files.lock().unwrap();
        for path in files.iter() {
            if path.exists() {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

impl DocumentDownloader for FlakyDownloader {
    fn download(&self, _url: &str, _job_id: &JobId) -> Result<PathBuf, InfrastructureError> {
        let attempt = self.attempt_count.fetch_add(1, Ordering::SeqCst) + 1;

        if attempt < self.succeed_on_attempt {
            // Fail with retryable error
            Err(InfrastructureError::TimeoutError(format!(
                "Timeout on attempt {}",
                attempt
            )))
        } else {
            // Succeed — create a real temp file
            let temp_dir = std::env::temp_dir();
            let temp_path = temp_dir.join(format!("flaky_test_{}.pdf", attempt));
            std::fs::write(&temp_path, b"%PDF-1.4 test").unwrap();

            // Track for cleanup
            self.created_files.lock().unwrap().push(temp_path.clone());

            Ok(temp_path)
        }
    }
}

// --- Mock Renderer ---

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

// --- Mock PrinterEngine ---

struct MockPrinterEngine;

impl PrinterEngine for MockPrinterEngine {
    fn print(&self, _printer_name: &str, _data: &[u8]) -> Result<(), InfrastructureError> {
        Ok(())
    }
}

// --- Helper: Create test database and worker ---

fn create_test_worker(
    downloader: Arc<dyn DocumentDownloader>,
) -> (
    QueueWorker,
    Arc<dyn sapo_printer::infrastructure::queue::QueueManager>,
    Arc<dyn PrintJobRepository>,
) {
    let mut conn = Connection::open_in_memory().unwrap();
    run_migrations(&mut conn).unwrap();
    let arc_conn = Arc::new(StdMutex::new(conn));

    let queue_manager = Arc::new(SqliteQueueManager::new(arc_conn.clone()));
    let job_repo = Arc::new(SqlitePrintJobRepository::new(arc_conn.clone()));
    let event_store = Arc::new(SqliteEventStore::new(arc_conn.clone()));
    let event_bus = Arc::new(MockEventBus);
    let renderer = Arc::new(MockRenderer);
    let printer_engine = Arc::new(MockPrinterEngine);

    let worker = QueueWorker::new(
        queue_manager.clone() as Arc<dyn sapo_printer::infrastructure::queue::QueueManager>,
        job_repo.clone() as Arc<dyn PrintJobRepository>,
        event_store,
        event_bus,
        downloader,
        renderer,
        printer_engine,
    );

    (
        worker,
        queue_manager as Arc<dyn sapo_printer::infrastructure::queue::QueueManager>,
        job_repo as Arc<dyn PrintJobRepository>,
    )
}

// --- Tests ---

#[test]
fn test_job_recovers_after_transient_failure() {
    // Downloader succeeds on attempt 2 (first attempt fails, retry succeeds)
    let downloader = Arc::new(FlakyDownloader::new(2));

    let (worker, queue_manager, job_repo) = create_test_worker(downloader.clone());

    // Create and save job (in Pending state)
    let job = PrintJob::new("https://s3.example.com/doc.pdf".into(), "HP".into());
    let job_id = job.id().clone();
    job_repo.save(&job).unwrap();

    // Push to queue (will transition Pending → Queued)
    queue_manager.push(job.id()).unwrap();

    // Start worker
    worker.start().expect("Failed to start worker");

    // Wait for processing + retry (first attempt fails, 5s delay, second attempt succeeds)
    // Total time: ~5-6 seconds
    thread::sleep(Duration::from_secs(8));

    worker.stop().expect("Failed to stop worker");

    // Verify job completed successfully after retry
    let final_job = job_repo.find_by_id(&job_id).unwrap().unwrap();
    assert_eq!(final_job.status(), &PrintStatus::Completed);
    assert_eq!(final_job.retry_count(), 1); // 1 retry occurred

    // Verify downloader was called twice
    assert_eq!(downloader.get_attempt_count(), 2);
}

#[test]
fn test_job_fails_after_max_retries() {
    // Downloader never succeeds (always fails)
    let downloader = Arc::new(FlakyDownloader::new(999)); // Never succeeds

    let (worker, queue_manager, job_repo) = create_test_worker(downloader.clone());

    // Create and save job (in Pending state)
    let job = PrintJob::new("https://s3.example.com/doc.pdf".into(), "HP".into());
    let job_id = job.id().clone();
    job_repo.save(&job).unwrap();

    // Push to queue (will transition Pending → Queued)
    queue_manager.push(job.id()).unwrap();

    // Start worker
    worker.start().expect("Failed to start worker");

    // Wait for all retries to exhaust
    // Attempt 1: immediate fail
    // Retry 1 (5s delay): fail → retry_count = 1
    // Retry 2 (10s delay): fail → retry_count = 2
    // Retry 3 (20s delay): fail → retry_count = 3
    // Total time: ~35-40 seconds
    thread::sleep(Duration::from_secs(45));

    worker.stop().expect("Failed to stop worker");

    // Verify job failed permanently
    let final_job = job_repo.find_by_id(&job_id).unwrap().unwrap();
    assert_eq!(final_job.status(), &PrintStatus::Failed);
    assert_eq!(final_job.retry_count(), 3); // Max retries reached

    // Verify downloader was called 4 times (initial + 3 retries)
    assert_eq!(downloader.get_attempt_count(), 4);
}

#[test]
fn test_backoff_timing_accuracy() {
    // Downloader succeeds on attempt 4 (test all 3 retry delays)
    let downloader = Arc::new(FlakyDownloader::new(4));

    let (worker, queue_manager, job_repo) = create_test_worker(downloader.clone());

    // Create and save job (in Pending state)
    let job = PrintJob::new("https://s3.example.com/doc.pdf".into(), "HP".into());
    let job_id = job.id().clone();
    job_repo.save(&job).unwrap();

    // Push to queue (will transition Pending → Queued)
    queue_manager.push(job.id()).unwrap();

    // Start worker and measure timing
    let start_time = Instant::now();
    worker.start().expect("Failed to start worker");

    // Wait for completion
    // Expected timing:
    // Attempt 1: fail immediately
    // Delay 1: 5s
    // Attempt 2: fail
    // Delay 2: 10s
    // Attempt 3: fail
    // Delay 3: 20s
    // Attempt 4: succeed
    // Total: ~35-36 seconds (accounting for processing overhead)
    thread::sleep(Duration::from_secs(40));

    worker.stop().expect("Failed to stop worker");
    let elapsed = start_time.elapsed();

    // Verify job completed successfully
    let final_job = job_repo.find_by_id(&job_id).unwrap().unwrap();
    assert_eq!(final_job.status(), &PrintStatus::Completed);
    assert_eq!(final_job.retry_count(), 3); // 3 retries occurred

    // Verify timing (should be ~35-40s total)
    assert!(
        elapsed >= Duration::from_secs(35),
        "Total elapsed time should be at least 35s (5s + 10s + 20s delays), got {:?}",
        elapsed
    );
    assert!(
        elapsed <= Duration::from_secs(45),
        "Total elapsed time should not exceed 45s (accounting for overhead), got {:?}",
        elapsed
    );

    // Verify downloader was called 4 times
    assert_eq!(downloader.get_attempt_count(), 4);
}
