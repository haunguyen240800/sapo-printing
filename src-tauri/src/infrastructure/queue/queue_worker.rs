//! Queue Worker — Background Job Processor
//!
//! Implements a background worker thread that continuously polls the queue
//! and processes print jobs through the complete pipeline:
//! pop → download → render → print → complete.
//!
//! ## Architecture
//! - **Pattern:** Worker Pattern + Background Processing
//! - **Layer:** Infrastructure (Queue Management)
//! - **Thread Safety:** All dependencies wrapped in Arc, AtomicBool for lifecycle control
//! - **Cleanup:** RAII via TempPdfFile Drop trait (automatic temp file cleanup)
//!
//! ## Lifecycle
//! 1. `start()` — spawns background thread, begins polling
//! 2. `process_loop()` — polls queue every 500ms, processes jobs sequentially
//! 3. `stop()` — sets running=false, waits for graceful shutdown
//!
//! ## Event-Driven Architecture
//! Worker publishes 5 events per job:
//! - PrintJobQueued (PENDING → QUEUED)
//! - PrintJobDownloaded (QUEUED → DOWNLOADED)
//! - PrintJobSubmitted (DOWNLOADED → SUBMITTED_TO_QUEUE)
//! - PrintJobPrinting (SUBMITTED_TO_QUEUE → PRINTING)
//! - PrintJobCompleted (PRINTING → COMPLETED)
//!
//! ## Story Context
//! This is Story 3.5 — implements sequential job processing only.
//! - ✅ Single worker thread
//! - ✅ FIFO queue processing
//! - ✅ State transitions + event publishing
//! - ❌ Auto-retry logic (Story 3.6)
//! - ❌ Job cancellation (Story 3.7)
//! - ❌ Concurrent workers (Story 4.x)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::domain::print_job::aggregate::PrintJob;
use crate::domain::print_job::PrintJobRepository;
use crate::infrastructure::database::SqliteEventStore;
use crate::infrastructure::downloader::DocumentDownloader;
use crate::infrastructure::printer::PrinterEngine;
use crate::infrastructure::queue::QueueManager;
use crate::infrastructure::renderer::{DocumentRenderer, RenderConfig};
use crate::infrastructure::temp_file::TempPdfFile;
use crate::shared::event_bus::EventBus;

/// Background worker for processing print jobs from the queue.
///
/// Polls the queue every 500ms, processes jobs sequentially through
/// the complete pipeline, and publishes events at each state transition.
///
/// # Thread Safety
/// All fields are Arc-wrapped for safe sharing across threads.
/// The `running` flag uses AtomicBool for lock-free lifecycle control.
pub struct QueueWorker {
    queue_manager: Arc<dyn QueueManager>,
    job_repo: Arc<dyn PrintJobRepository>,
    event_store: Arc<SqliteEventStore>,
    event_bus: Arc<dyn EventBus>,
    downloader: Arc<dyn DocumentDownloader>,
    renderer: Arc<dyn DocumentRenderer>,
    printer_engine: Arc<dyn PrinterEngine>,

    // Worker lifecycle control
    running: Arc<AtomicBool>,
    thread_handle: Mutex<Option<JoinHandle<()>>>,
}

impl QueueWorker {
    /// Creates a new QueueWorker with the given dependencies.
    ///
    /// The worker is created in a stopped state. Call `start()` to begin processing.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        queue_manager: Arc<dyn QueueManager>,
        job_repo: Arc<dyn PrintJobRepository>,
        event_store: Arc<SqliteEventStore>,
        event_bus: Arc<dyn EventBus>,
        downloader: Arc<dyn DocumentDownloader>,
        renderer: Arc<dyn DocumentRenderer>,
        printer_engine: Arc<dyn PrinterEngine>,
    ) -> Self {
        Self {
            queue_manager,
            job_repo,
            event_store,
            event_bus,
            downloader,
            renderer,
            printer_engine,
            running: Arc::new(AtomicBool::new(false)),
            thread_handle: Mutex::new(None),
        }
    }

    /// Start the worker in a background thread.
    ///
    /// The worker will poll the queue every 500ms and process jobs sequentially.
    /// Returns an error if the worker is already running.
    ///
    /// # Errors
    /// Returns `Err` if the worker is already running.
    pub fn start(&self) -> Result<(), String> {
        // Atomically check-and-set running flag (prevents TOCTOU race)
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("Worker already running".into());
        }

        // Clone Arc references for thread
        let queue_manager = Arc::clone(&self.queue_manager);
        let job_repo = Arc::clone(&self.job_repo);
        let event_store = Arc::clone(&self.event_store);
        let event_bus = Arc::clone(&self.event_bus);
        let downloader = Arc::clone(&self.downloader);
        let renderer = Arc::clone(&self.renderer);
        let printer_engine = Arc::clone(&self.printer_engine);
        let running = Arc::clone(&self.running);

        // Spawn background thread
        let handle = thread::spawn(move || {
            Self::process_loop(
                queue_manager,
                job_repo,
                event_store,
                event_bus,
                downloader,
                renderer,
                printer_engine,
                running,
            );
        });

        // Store handle (recover from mutex poison)
        *self.thread_handle.lock().unwrap_or_else(|p| p.into_inner()) = Some(handle);

        Ok(())
    }

    /// Stop the worker gracefully.
    ///
    /// Sets the running flag to false and waits for the worker thread to finish
    /// processing the current job (if any) before returning.
    ///
    /// # Errors
    /// Returns `Err` if the worker thread panicked.
    pub fn stop(&self) -> Result<(), String> {
        // Set running flag to false
        self.running.store(false, Ordering::SeqCst);

        // Wait for thread to finish (recover from mutex poison)
        let mut handle_guard = self.thread_handle.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(handle) = handle_guard.take() {
            handle.join().map_err(|_| "Failed to join worker thread")?;
        }

        Ok(())
    }

    /// Check if the worker is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Main processing loop — runs in background thread.
    ///
    /// Polls the queue every 500ms, processes jobs sequentially, and handles errors gracefully.
    #[allow(clippy::too_many_arguments)]
    fn process_loop(
        queue_manager: Arc<dyn QueueManager>,
        job_repo: Arc<dyn PrintJobRepository>,
        event_store: Arc<SqliteEventStore>,
        event_bus: Arc<dyn EventBus>,
        downloader: Arc<dyn DocumentDownloader>,
        renderer: Arc<dyn DocumentRenderer>,
        printer_engine: Arc<dyn PrinterEngine>,
        running: Arc<AtomicBool>,
    ) {
        const POLL_INTERVAL_MS: u64 = 500; // 0.5s poll interval

        tracing::info!(
            target = "sapo_printer::queue_worker",
            "QueueWorker: processing loop started"
        );

        while running.load(Ordering::SeqCst) {
            match queue_manager.pop() {
                Ok(Some(job)) => {
                    let job_id = job.id().clone(); // Save ID before consuming job

                    tracing::info!(
                        target = "sapo_printer::queue_worker",
                        job_id = %job_id,
                        "QueueWorker: picked up job for processing"
                    );

                    // Process job through pipeline
                    let result = Self::process_job(
                        job,
                        &job_repo,
                        &event_store,
                        &event_bus,
                        &downloader,
                        &renderer,
                        &printer_engine,
                    );

                    if let Err(e) = result {
                        tracing::error!(
                            target = "sapo_printer::queue_worker",
                            job_id = %job_id,
                            error = e,
                            "QueueWorker: job processing failed"
                        );
                        // Load job from repo to get latest state after process_job mutations
                        if let Ok(Some(failed_job)) = job_repo.find_by_id(&job_id) {
                            Self::handle_job_failure(
                                e,
                                failed_job,
                                &queue_manager,
                                &job_repo,
                                &event_store,
                                &event_bus,
                            );
                        } else {
                            tracing::error!(
                                target = "sapo_printer::queue_worker",
                                job_id = %job_id,
                                "QueueWorker: Could not load job for retry handling"
                            );
                        }
                    } else {
                        tracing::info!(
                            target = "sapo_printer::queue_worker",
                            job_id = %job_id,
                            "QueueWorker: job completed successfully"
                        );
                    }
                }
                Ok(None) => {
                    // Queue empty, sleep before next poll
                    thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
                }
                Err(e) => {
                    eprintln!("Worker: queue pop failed: {}", e);
                    thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
                }
            }
        }
    }

    /// Process a single job through the complete pipeline.
    ///
    /// Pipeline steps:
    /// 1. PENDING → QUEUED (transition from pop state)
    /// 2. Download document
    /// 3. QUEUED → DOWNLOADED
    /// 4. Render document
    /// 5. DOWNLOADED → SUBMITTED_TO_QUEUE
    /// 6. SUBMITTED_TO_QUEUE → PRINTING
    /// 7. Send to printer
    /// 8. PRINTING → COMPLETED
    ///
    /// Each step persists the job state and publishes domain events.
    /// Temp files are auto-cleaned via RAII (TempPdfFile Drop).
    ///
    /// # Errors
    /// Returns `Err` if any step fails. The job remains in the last successful state.
    /// Story 3.6 will add retry logic for transient failures.
    #[allow(clippy::too_many_arguments)]
    fn process_job(
        mut job: PrintJob,
        job_repo: &Arc<dyn PrintJobRepository>,
        event_store: &SqliteEventStore,
        event_bus: &Arc<dyn EventBus>,
        downloader: &Arc<dyn DocumentDownloader>,
        renderer: &Arc<dyn DocumentRenderer>,
        printer_engine: &Arc<dyn PrinterEngine>,
    ) -> Result<(), String> {
        // Step 1: Transition to Queued (pop returns Pending, we transition to Queued)
        job.queue()
            .map_err(|e| format!("Failed to queue job: {:?}", e))?;
        if let Err(e) = Self::persist_and_publish(&mut job, job_repo, event_store, event_bus) {
            // Persist failed — job is stuck in Pending in DB.
            // Mark as Failed so it's not orphaned (pop only selects Queued rows).
            eprintln!(
                "Worker: initial persist failed for job {}, marking as Failed: {}",
                job.id(),
                e
            );
            let _ = job.fail(format!("Initial persist failed: {}", e));
            let _ = job_repo.update(&job);
            return Err(e);
        }

        // Step 2: Download document
        let download_start = std::time::Instant::now();
        let pdf_path = downloader
            .download(job.pdf_url(), job.id())
            .map_err(|e| format!("Download failed: {:?}", e))?;
        let download_duration = download_start.elapsed();
        tracing::info!(
            target = "sapo_printer::metrics",
            job_id = %job.id(),
            step = "download",
            duration_ms = download_duration.as_millis() as u64,
            "Pipeline step completed"
        );

        // Wrap in TempPdfFile for RAII cleanup (auto-deletes on drop)
        let temp_file = TempPdfFile::new(pdf_path);

        job.mark_downloaded()
            .map_err(|e| format!("Failed to mark downloaded: {:?}", e))?;
        Self::persist_and_publish(&mut job, job_repo, event_store, event_bus)?;

        // Step 3: Render document
        let render_start = std::time::Instant::now();
        let render_config = RenderConfig::default();
        let rendered_data = renderer
            .render(temp_file.path(), &render_config)
            .map_err(|e| format!("Render failed: {:?}", e))?;
        let render_duration = render_start.elapsed();
        tracing::info!(
            target = "sapo_printer::metrics",
            job_id = %job.id(),
            step = "render",
            duration_ms = render_duration.as_millis() as u64,
            "Pipeline step completed"
        );

        job.mark_submitted()
            .map_err(|e| format!("Failed to mark submitted: {:?}", e))?;
        Self::persist_and_publish(&mut job, job_repo, event_store, event_bus)?;

        // Step 4: Send to printer
        job.mark_printing()
            .map_err(|e| format!("Failed to mark printing: {:?}", e))?;
        Self::persist_and_publish(&mut job, job_repo, event_store, event_bus)?;

        let print_start = std::time::Instant::now();
        printer_engine
            .print(job.printer_name(), &rendered_data)
            .map_err(|e| format!("Print failed: {:?}", e))?;
        let print_duration = print_start.elapsed();
        tracing::info!(
            target = "sapo_printer::metrics",
            job_id = %job.id(),
            step = "print",
            duration_ms = print_duration.as_millis() as u64,
            "Pipeline step completed"
        );

        // Step 5: Mark complete
        job.complete()
            .map_err(|e| format!("Failed to mark complete: {:?}", e))?;
        Self::persist_and_publish(&mut job, job_repo, event_store, event_bus)?;

        // Step 6: temp_file is dropped here → RAII cleanup deletes the file

        Ok(())
    }

    /// Persist job state and publish domain events.
    ///
    /// Implements the Outbox Pattern:
    /// 1. Drain events from aggregate
    /// 2. Persist job state to repository
    /// 3. Persist events to event store
    /// 4. Publish events to event bus (non-fatal)
    ///
    /// This pattern ensures consistency: if the DB write succeeds, events are persisted
    /// even if the EventBus publish fails. Event bus failures are logged but don't stop processing.
    fn persist_and_publish(
        job: &mut PrintJob,
        job_repo: &Arc<dyn PrintJobRepository>,
        event_store: &SqliteEventStore,
        event_bus: &Arc<dyn EventBus>,
    ) -> Result<(), String> {
        // 1. Drain events from aggregate
        let events = job.drain_events();

        // 2. Persist events to event store FIRST
        // If this fails, job state is not updated — consistent (no change).
        // Events are drained but the job stays in its old state and can be retried.
        event_store
            .save_all(job.id().to_string().as_str(), &events)
            .map_err(|e| format!("Failed to save events: {:?}", e))?;

        // 3. Persist job state
        // If this fails after events were saved, events exist in store but job
        // state is not updated — recoverable via event replay.
        job_repo
            .update(job)
            .map_err(|e| format!("Failed to update job: {:?}", e))?;

        // 4. Publish events to event bus (non-fatal)
        for event in &events {
            let payload = event.serialize_payload();
            if let Err(e) = event_bus.publish(event.event_type(), &payload) {
                eprintln!(
                    "Warning: Failed to publish event {}: {}",
                    event.event_type(),
                    e
                );
                // Non-fatal — continue processing
            }
        }

        Ok(())
    }

    /// Handle job failure with retry logic.
    ///
    /// Decision tree:
    /// 1. Mark job as FAILED first
    /// 2. Parse error string to determine if retryable
    /// 3. If retryable AND retry_count < 3:
    ///    - Call job.retry() (increments retry_count, FAILED → QUEUED)
    ///    - Calculate backoff delay
    ///    - Call queue_manager.requeue(job_id, delay)
    ///    - Persist and publish events
    /// 4. If non-retryable OR retry_count >= 3:
    ///    - Job stays FAILED permanently
    ///    - Persist and publish PrintJobFailed event
    fn handle_job_failure(
        error: String,
        mut job: PrintJob,
        queue_manager: &Arc<dyn QueueManager>,
        job_repo: &Arc<dyn PrintJobRepository>,
        event_store: &Arc<SqliteEventStore>,
        event_bus: &Arc<dyn EventBus>,
    ) {
        use crate::infrastructure::queue::retry_logic::calculate_backoff_delay;

        // Step 1: Mark job as FAILED first (required for retry() to work)
        if let Err(e) = job.fail(error.clone()) {
            eprintln!("Worker: Failed to mark job {} as failed: {:?}", job.id(), e);
            return;
        }

        // Step 2: Parse error string to determine if retryable (best-effort)
        let is_retryable_error = error.contains("timeout")
            || error.contains("Timeout")
            || error.contains("Network")
            || error.contains("Render failed")
            || error.contains("Printer error");

        let can_retry = is_retryable_error && job.retry_count() < 3;

        if can_retry {
            // Step 3: Attempt retry
            // Calculate delay BEFORE retry() to avoid off-by-one dependency
            let delay = calculate_backoff_delay(job.retry_count());

            match job.retry() {
                Ok(()) => {
                    eprintln!(
                        "Worker: Job {} failed (attempt {}), retrying in {}s: {}",
                        job.id(),
                        job.retry_count(),
                        delay,
                        error
                    );

                    // Requeue with delay
                    if let Err(e) = queue_manager.requeue(job.id(), delay) {
                        eprintln!("Worker: Failed to requeue job {}: {}", job.id(), e);
                        // Rollback: mark job as FAILED since requeue failed
                        let _ = job.fail(format!("Requeue failed: {}", e));
                        let _ =
                            Self::persist_and_publish(&mut job, job_repo, event_store, event_bus);
                        return;
                    }

                    // Persist state and publish events
                    if let Err(e) =
                        Self::persist_and_publish(&mut job, job_repo, event_store, event_bus)
                    {
                        eprintln!(
                            "Worker: Failed to persist retry for job {}: {}",
                            job.id(),
                            e
                        );
                    }
                }
                Err(e) => {
                    eprintln!(
                        "Worker: Cannot retry job {} (retry_count={}): {:?}",
                        job.id(),
                        job.retry_count(),
                        e
                    );
                    // Job is already FAILED, persist with retry
                    for attempt in 0..2 {
                        match Self::persist_and_publish(&mut job, job_repo, event_store, event_bus)
                        {
                            Ok(()) => break,
                            Err(e) if attempt == 0 => {
                                eprintln!(
                                    "Worker: Persist failed, retrying once for job {}: {}",
                                    job.id(),
                                    e
                                );
                                std::thread::sleep(std::time::Duration::from_millis(100));
                            }
                            Err(e) => {
                                eprintln!(
                                    "Worker: Failed to persist failed job {} after retry: {}",
                                    job.id(),
                                    e
                                );
                            }
                        }
                    }
                }
            }
        } else {
            // Step 4: Non-retryable or exhausted retries - job stays FAILED
            let reason = if job.retry_count() >= 3 {
                format!("Max retries exceeded (3): {}", error)
            } else {
                format!("Non-retryable error: {}", error)
            };

            eprintln!("Worker: Job {} failed permanently: {}", job.id(), reason);

            // Job is already FAILED, just persist
            if let Err(e) = Self::persist_and_publish(&mut job, job_repo, event_store, event_bus) {
                eprintln!("Worker: Failed to persist failed job {}: {}", job.id(), e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::sync::Mutex as StdMutex;

    use crate::domain::print_job::errors::DomainError;
    use crate::domain::print_job::value_objects::{JobId, PrintStatus};
    use crate::domain::print_job::PrintJobRepository;
    use crate::infrastructure::database::{run_migrations, SqliteEventStore};
    use crate::infrastructure::queue::QueueError;
    use crate::infrastructure::secrets::SecretManager;
    use crate::shared::errors::InfrastructureError;
    use crate::shared::event_bus::EventBusError;
    use std::collections::HashMap;

    struct MockSecretManager {
        store: StdMutex<HashMap<String, String>>,
    }

    impl MockSecretManager {
        fn new() -> Self {
            Self {
                store: StdMutex::new(HashMap::new()),
            }
        }
    }

    impl SecretManager for MockSecretManager {
        fn store(&self, key: &str, value: &str) -> Result<(), InfrastructureError> {
            self.store
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_string());
            Ok(())
        }

        fn retrieve(&self, key: &str) -> Result<Option<String>, InfrastructureError> {
            Ok(self.store.lock().unwrap().get(key).cloned())
        }

        fn delete(&self, key: &str) -> Result<(), InfrastructureError> {
            self.store.lock().unwrap().remove(key);
            Ok(())
        }
    }

    // --- Mock QueueManager ---

    struct MockQueueManager {
        jobs: StdMutex<Vec<PrintJob>>,
        requeue_calls: StdMutex<Vec<(JobId, u64)>>, // (job_id, delay_secs)
    }

    impl MockQueueManager {
        fn new() -> Self {
            Self {
                jobs: StdMutex::new(Vec::new()),
                requeue_calls: StdMutex::new(Vec::new()),
            }
        }

        fn push_job(&self, job: PrintJob) {
            self.jobs.lock().unwrap().push(job);
        }

        fn get_requeue_calls(&self) -> Vec<(JobId, u64)> {
            self.requeue_calls.lock().unwrap().clone()
        }
    }

    impl QueueManager for MockQueueManager {
        fn push(&self, _job_id: &JobId) -> Result<(), QueueError> {
            Ok(())
        }

        fn pop(&self) -> Result<Option<PrintJob>, QueueError> {
            Ok(self.jobs.lock().unwrap().pop())
        }

        fn requeue(&self, job_id: &JobId, delay_secs: u64) -> Result<(), QueueError> {
            self.requeue_calls
                .lock()
                .unwrap()
                .push((job_id.clone(), delay_secs));
            Ok(())
        }

        fn queue_depth(&self) -> Result<usize, QueueError> {
            Ok(self.jobs.lock().unwrap().len())
        }
    }

    // --- Mock PrintJobRepository ---

    struct MockPrintJobRepository {
        jobs: StdMutex<std::collections::HashMap<JobId, PrintJob>>,
        updates: StdMutex<Vec<JobId>>,
    }

    impl MockPrintJobRepository {
        fn new() -> Self {
            Self {
                jobs: StdMutex::new(std::collections::HashMap::new()),
                updates: StdMutex::new(Vec::new()),
            }
        }

        fn add_job(&self, job: PrintJob) {
            self.jobs.lock().unwrap().insert(job.id().clone(), job);
        }

        fn update_count(&self) -> usize {
            self.updates.lock().unwrap().len()
        }
    }

    impl PrintJobRepository for MockPrintJobRepository {
        fn save(&self, job: &PrintJob) -> Result<(), DomainError> {
            self.jobs
                .lock()
                .unwrap()
                .insert(job.id().clone(), job.clone());
            Ok(())
        }

        fn update(&self, job: &PrintJob) -> Result<(), DomainError> {
            self.updates.lock().unwrap().push(job.id().clone());
            self.jobs
                .lock()
                .unwrap()
                .insert(job.id().clone(), job.clone());
            Ok(())
        }

        fn find_by_id(&self, id: &JobId) -> Result<Option<PrintJob>, DomainError> {
            Ok(self.jobs.lock().unwrap().get(id).cloned())
        }

        fn find_by_status(&self, _status: &PrintStatus) -> Result<Vec<PrintJob>, DomainError> {
            unimplemented!()
        }

        fn find_all(&self) -> Result<Vec<PrintJob>, DomainError> {
            unimplemented!()
        }
    }

    // --- Mock EventStore ---

    // Removed MockEventStore - using real SqliteEventStore with in-memory DB instead

    // --- Mock EventBus ---

    struct MockEventBus {
        published: StdMutex<Vec<String>>,
    }

    impl MockEventBus {
        fn new() -> Self {
            Self {
                published: StdMutex::new(Vec::new()),
            }
        }

        fn publish_count(&self) -> usize {
            self.published.lock().unwrap().len()
        }
    }

    impl EventBus for MockEventBus {
        fn publish(&self, event_type: &str, _payload: &str) -> Result<(), EventBusError> {
            self.published.lock().unwrap().push(event_type.to_string());
            Ok(())
        }
    }

    // --- Mock DocumentDownloader ---

    struct MockDownloader {
        should_fail: StdMutex<bool>,
        return_path: StdMutex<Option<std::path::PathBuf>>,
    }

    impl MockDownloader {
        fn new() -> Self {
            Self {
                should_fail: StdMutex::new(false),
                return_path: StdMutex::new(None),
            }
        }

        fn set_fail(&self, fail: bool) {
            *self.should_fail.lock().unwrap() = fail;
        }

        fn set_return_path(&self, path: std::path::PathBuf) {
            *self.return_path.lock().unwrap() = Some(path);
        }
    }

    impl DocumentDownloader for MockDownloader {
        fn download(
            &self,
            _url: &str,
            _job_id: &JobId,
        ) -> Result<std::path::PathBuf, InfrastructureError> {
            if *self.should_fail.lock().unwrap() {
                return Err(InfrastructureError::NetworkError(
                    "Mock download failure".to_string(),
                ));
            }
            let guard = self.return_path.lock().unwrap();
            if let Some(path) = guard.as_ref() {
                Ok(path.clone())
            } else {
                Ok(std::path::PathBuf::from("/tmp/mock.pdf"))
            }
        }
    }

    // Mock Downloader that fails with configurable error
    struct FailingDownloader {
        error_message: String,
    }

    impl FailingDownloader {
        fn new(error_message: String) -> Self {
            Self { error_message }
        }
    }

    impl DocumentDownloader for FailingDownloader {
        fn download(
            &self,
            _url: &str,
            _job_id: &JobId,
        ) -> Result<std::path::PathBuf, InfrastructureError> {
            Err(InfrastructureError::TimeoutError(
                self.error_message.clone(),
            ))
        }
    }

    // --- Mock DocumentRenderer ---

    struct MockRenderer {
        should_fail: StdMutex<bool>,
    }

    impl MockRenderer {
        fn new() -> Self {
            Self {
                should_fail: StdMutex::new(false),
            }
        }

        fn set_fail(&self, fail: bool) {
            *self.should_fail.lock().unwrap() = fail;
        }
    }

    impl DocumentRenderer for MockRenderer {
        fn render(
            &self,
            _path: &std::path::Path,
            _config: &RenderConfig,
        ) -> Result<Vec<u8>, InfrastructureError> {
            if *self.should_fail.lock().unwrap() {
                Err(InfrastructureError::RenderError(
                    "Mock render failure".to_string(),
                ))
            } else {
                Ok(vec![0x25, 0x50, 0x44, 0x46]) // Mock PDF data
            }
        }
    }

    // --- Mock PrinterEngine ---

    struct MockPrinterEngine {
        should_fail: StdMutex<bool>,
    }

    impl MockPrinterEngine {
        fn new() -> Self {
            Self {
                should_fail: StdMutex::new(false),
            }
        }

        fn set_fail(&self, fail: bool) {
            *self.should_fail.lock().unwrap() = fail;
        }
    }

    impl PrinterEngine for MockPrinterEngine {
        fn print(&self, _printer_name: &str, _data: &[u8]) -> Result<(), InfrastructureError> {
            if *self.should_fail.lock().unwrap() {
                Err(InfrastructureError::PrinterError {
                    reason: "Mock print failure".to_string(),
                })
            } else {
                Ok(())
            }
        }
    }

    // --- Helper: Create worker with mocks ---

    fn create_test_worker(
        queue_manager: Arc<MockQueueManager>,
        job_repo: Arc<MockPrintJobRepository>,
        event_store: Arc<SqliteEventStore>,
        event_bus: Arc<MockEventBus>,
        downloader: Arc<MockDownloader>,
        renderer: Arc<MockRenderer>,
        printer_engine: Arc<MockPrinterEngine>,
    ) -> QueueWorker {
        QueueWorker::new(
            queue_manager as Arc<dyn QueueManager>,
            job_repo as Arc<dyn PrintJobRepository>,
            event_store,
            event_bus as Arc<dyn EventBus>,
            downloader as Arc<dyn DocumentDownloader>,
            renderer as Arc<dyn DocumentRenderer>,
            printer_engine as Arc<dyn PrinterEngine>,
        )
    }

    // --- Helper: Create in-memory DB with event store ---

    fn create_test_event_store() -> Arc<SqliteEventStore> {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();
        Arc::new(SqliteEventStore::new(Arc::new(Mutex::new(conn)), Arc::new(MockSecretManager::new())))
    }

    // --- Tests ---

    #[test]
    fn test_worker_starts_and_stops() {
        let queue_manager = Arc::new(MockQueueManager::new());
        let job_repo = Arc::new(MockPrintJobRepository::new());
        let event_store = create_test_event_store();
        let event_bus = Arc::new(MockEventBus::new());
        let downloader = Arc::new(MockDownloader::new());
        let renderer = Arc::new(MockRenderer::new());
        let printer_engine = Arc::new(MockPrinterEngine::new());

        let worker = create_test_worker(
            queue_manager,
            job_repo,
            event_store,
            event_bus,
            downloader,
            renderer,
            printer_engine,
        );

        // Initially not running
        assert!(!worker.is_running());

        // Start worker
        worker.start().expect("Failed to start worker");
        assert!(worker.is_running());

        // Cannot start twice
        assert!(worker.start().is_err());

        // Stop worker
        worker.stop().expect("Failed to stop worker");
        assert!(!worker.is_running());
    }

    #[test]
    fn test_worker_processes_single_job() {
        let queue_manager = Arc::new(MockQueueManager::new());
        let job_repo = Arc::new(MockPrintJobRepository::new());
        let event_store = create_test_event_store();
        let event_bus = Arc::new(MockEventBus::new());
        let downloader = Arc::new(MockDownloader::new());
        let renderer = Arc::new(MockRenderer::new());
        let printer_engine = Arc::new(MockPrinterEngine::new());

        // Create and push a job
        let job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP".to_string(),
        );
        queue_manager.push_job(job);

        let worker = create_test_worker(
            Arc::clone(&queue_manager),
            Arc::clone(&job_repo),
            Arc::clone(&event_store),
            Arc::clone(&event_bus),
            downloader,
            renderer,
            printer_engine,
        );

        worker.start().expect("Failed to start worker");

        // Wait for processing (max 2s)
        thread::sleep(Duration::from_millis(1500));

        worker.stop().expect("Failed to stop worker");

        // Verify job was updated 5 times (Queued, Downloaded, Submitted, Printing, Completed)
        assert_eq!(job_repo.update_count(), 5);
    }

    #[test]
    fn test_worker_processes_multiple_jobs_fifo() {
        let queue_manager = Arc::new(MockQueueManager::new());
        let job_repo = Arc::new(MockPrintJobRepository::new());
        let event_store = create_test_event_store();
        let event_bus = Arc::new(MockEventBus::new());
        let downloader = Arc::new(MockDownloader::new());
        let renderer = Arc::new(MockRenderer::new());
        let printer_engine = Arc::new(MockPrinterEngine::new());

        // Push 3 jobs (Vec::pop returns last element, so reverse order for FIFO)
        let job3 = PrintJob::new(
            "https://s3.example.com/doc3.pdf".to_string(),
            "HP".to_string(),
        );
        let job2 = PrintJob::new(
            "https://s3.example.com/doc2.pdf".to_string(),
            "HP".to_string(),
        );
        let job1 = PrintJob::new(
            "https://s3.example.com/doc1.pdf".to_string(),
            "HP".to_string(),
        );

        queue_manager.push_job(job3);
        queue_manager.push_job(job2);
        queue_manager.push_job(job1);

        let worker = create_test_worker(
            Arc::clone(&queue_manager),
            Arc::clone(&job_repo),
            Arc::clone(&event_store),
            Arc::clone(&event_bus),
            downloader,
            renderer,
            printer_engine,
        );

        worker.start().expect("Failed to start worker");

        // Wait for processing (max 3s)
        thread::sleep(Duration::from_millis(2500));

        worker.stop().expect("Failed to stop worker");

        // Verify all 3 jobs processed (3 jobs × 5 updates each = 15)
        assert_eq!(job_repo.update_count(), 15);
    }

    #[test]
    fn test_worker_sleeps_when_queue_empty() {
        let queue_manager = Arc::new(MockQueueManager::new());
        let job_repo = Arc::new(MockPrintJobRepository::new());
        let event_store = create_test_event_store();
        let event_bus = Arc::new(MockEventBus::new());
        let downloader = Arc::new(MockDownloader::new());
        let renderer = Arc::new(MockRenderer::new());
        let printer_engine = Arc::new(MockPrinterEngine::new());

        // No jobs in queue

        let worker = create_test_worker(
            Arc::clone(&queue_manager),
            Arc::clone(&job_repo),
            event_store,
            event_bus,
            downloader,
            renderer,
            printer_engine,
        );

        worker.start().expect("Failed to start worker");

        // Run for 1.5s with empty queue — verify timing (not busy-loop)
        let start_time = std::time::Instant::now();
        thread::sleep(Duration::from_millis(1500));
        let elapsed = start_time.elapsed();

        worker.stop().expect("Failed to stop worker");

        // Verify no jobs processed
        assert_eq!(job_repo.update_count(), 0);

        // Verify we didn't busy-loop: elapsed should be close to 1.5s
        // A busy-loop would complete in <100ms due to no actual work
        assert!(
            elapsed >= Duration::from_millis(1400),
            "Worker should sleep between polls, elapsed: {:?}",
            elapsed
        );
    }

    #[test]
    fn test_worker_publishes_events_at_each_step() {
        let queue_manager = Arc::new(MockQueueManager::new());
        let job_repo = Arc::new(MockPrintJobRepository::new());
        let event_store = create_test_event_store();
        let event_bus = Arc::new(MockEventBus::new());
        let downloader = Arc::new(MockDownloader::new());
        let renderer = Arc::new(MockRenderer::new());
        let printer_engine = Arc::new(MockPrinterEngine::new());

        let mut job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP".to_string(),
        );
        // Drain the PrintJobCreated event from new() so we only count worker events
        job.drain_events();
        queue_manager.push_job(job);

        let worker = create_test_worker(
            queue_manager,
            job_repo,
            Arc::clone(&event_store),
            Arc::clone(&event_bus),
            downloader,
            renderer,
            printer_engine,
        );

        worker.start().expect("Failed to start worker");
        thread::sleep(Duration::from_millis(1500));
        worker.stop().expect("Failed to stop worker");

        // Verify 5 events published: Queued, Downloaded, Submitted, Printing, Completed
        assert_eq!(event_bus.publish_count(), 5);
        // Note: Cannot easily verify event_store.event_count() without exposing query method
    }

    #[test]
    fn test_worker_persists_state_at_each_step() {
        let queue_manager = Arc::new(MockQueueManager::new());
        let job_repo = Arc::new(MockPrintJobRepository::new());
        let event_store = create_test_event_store();
        let event_bus = Arc::new(MockEventBus::new());
        let downloader = Arc::new(MockDownloader::new());
        let renderer = Arc::new(MockRenderer::new());
        let printer_engine = Arc::new(MockPrinterEngine::new());

        let job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP".to_string(),
        );
        queue_manager.push_job(job);

        let worker = create_test_worker(
            queue_manager,
            Arc::clone(&job_repo),
            event_store,
            event_bus,
            downloader,
            renderer,
            printer_engine,
        );

        worker.start().expect("Failed to start worker");
        thread::sleep(Duration::from_millis(1500));
        worker.stop().expect("Failed to stop worker");

        // Verify job_repo.update() called 5 times
        assert_eq!(job_repo.update_count(), 5);
    }

    #[test]
    fn test_worker_handles_download_failure() {
        let queue_manager = Arc::new(MockQueueManager::new());
        let job_repo = Arc::new(MockPrintJobRepository::new());
        let event_store = create_test_event_store();
        let event_bus = Arc::new(MockEventBus::new());
        let downloader = Arc::new(MockDownloader::new());
        let renderer = Arc::new(MockRenderer::new());
        let printer_engine = Arc::new(MockPrinterEngine::new());

        downloader.set_fail(true); // Cause download to fail

        let job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP".to_string(),
        );
        queue_manager.push_job(job);

        let worker = create_test_worker(
            queue_manager,
            Arc::clone(&job_repo),
            event_store,
            event_bus,
            downloader,
            renderer,
            printer_engine,
        );

        worker.start().expect("Failed to start worker");
        thread::sleep(Duration::from_millis(1500));
        worker.stop().expect("Failed to stop worker");

        // Job should only reach Queued state (1 update), then fail
        assert_eq!(job_repo.update_count(), 1);
    }

    #[test]
    fn test_worker_handles_render_failure() {
        let queue_manager = Arc::new(MockQueueManager::new());
        let job_repo = Arc::new(MockPrintJobRepository::new());
        let event_store = create_test_event_store();
        let event_bus = Arc::new(MockEventBus::new());
        let downloader = Arc::new(MockDownloader::new());
        let renderer = Arc::new(MockRenderer::new());
        let printer_engine = Arc::new(MockPrinterEngine::new());

        renderer.set_fail(true); // Cause render to fail

        let job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP".to_string(),
        );
        queue_manager.push_job(job);

        let worker = create_test_worker(
            queue_manager,
            Arc::clone(&job_repo),
            event_store,
            event_bus,
            downloader,
            renderer,
            printer_engine,
        );

        worker.start().expect("Failed to start worker");
        thread::sleep(Duration::from_millis(1500));
        worker.stop().expect("Failed to stop worker");

        // Job should reach Downloaded state (2 updates: Queued, Downloaded), then fail
        assert_eq!(job_repo.update_count(), 2);
    }

    #[test]
    fn test_worker_handles_print_failure() {
        let queue_manager = Arc::new(MockQueueManager::new());
        let job_repo = Arc::new(MockPrintJobRepository::new());
        let event_store = create_test_event_store();
        let event_bus = Arc::new(MockEventBus::new());
        let downloader = Arc::new(MockDownloader::new());
        let renderer = Arc::new(MockRenderer::new());
        let printer_engine = Arc::new(MockPrinterEngine::new());

        printer_engine.set_fail(true); // Cause print to fail

        let job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP".to_string(),
        );
        queue_manager.push_job(job);

        let worker = create_test_worker(
            queue_manager,
            Arc::clone(&job_repo),
            event_store,
            event_bus,
            downloader,
            renderer,
            printer_engine,
        );

        worker.start().expect("Failed to start worker");
        thread::sleep(Duration::from_millis(1500));
        worker.stop().expect("Failed to stop worker");

        // Job should reach Printing state (4 updates: Queued, Downloaded, Submitted, Printing), then fail
        assert_eq!(job_repo.update_count(), 4);
    }

    #[test]
    fn test_worker_cleans_temp_file() {
        let queue_manager = Arc::new(MockQueueManager::new());
        let job_repo = Arc::new(MockPrintJobRepository::new());
        let event_store = create_test_event_store();
        let event_bus = Arc::new(MockEventBus::new());
        let downloader = Arc::new(MockDownloader::new());
        let renderer = Arc::new(MockRenderer::new());
        let printer_engine = Arc::new(MockPrinterEngine::new());

        // Create a real temp file that the mock downloader will "return"
        let temp_dir = std::env::temp_dir();
        let temp_file_path = temp_dir.join(format!(
            "sapo_worker_test_{}.pdf",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&temp_file_path, b"%PDF-1.4 test content").unwrap();
        assert!(
            temp_file_path.exists(),
            "Temp file should exist before processing"
        );

        // Configure mock to return the real temp file path
        downloader.set_return_path(temp_file_path.clone());

        let job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP".to_string(),
        );
        queue_manager.push_job(job);

        let worker = create_test_worker(
            queue_manager,
            Arc::clone(&job_repo),
            event_store,
            event_bus,
            downloader,
            renderer,
            printer_engine,
        );

        worker.start().expect("Failed to start worker");
        thread::sleep(Duration::from_millis(1500));
        worker.stop().expect("Failed to stop worker");

        // Verify job was processed
        assert_eq!(job_repo.update_count(), 5);

        // Verify temp file was cleaned up by TempPdfFile Drop
        assert!(
            !temp_file_path.exists(),
            "Temp file {:?} should be deleted after job processing",
            temp_file_path
        );
    }

    // --- Story 3.6: Retry Logic Tests ---

    #[test]
    fn test_retryable_error_triggers_retry_with_correct_delay() {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();
        let arc_conn = Arc::new(StdMutex::new(conn));

        let job_repo = Arc::new(MockPrintJobRepository::new());
        let queue_manager = Arc::new(MockQueueManager::new());
        let event_store = Arc::new(SqliteEventStore::new(arc_conn.clone(), Arc::new(MockSecretManager::new())));
        let event_bus = Arc::new(MockEventBus::new());

        // Create job in QUEUED state (normal state after pop from queue)
        let mut job = PrintJob::new("https://s3.example.com/doc.pdf".into(), "HP".into());
        let job_id = job.id().clone();
        job.queue().unwrap();
        job_repo.add_job(job.clone());

        // Simulate failure with retryable error
        let error_message = "Timeout after 30s".to_string();

        // Call handle_job_failure directly (it will mark job as FAILED first)
        QueueWorker::handle_job_failure(
            error_message,
            job,
            &(queue_manager.clone() as Arc<dyn QueueManager>),
            &(job_repo.clone() as Arc<dyn PrintJobRepository>),
            &event_store,
            &(event_bus.clone() as Arc<dyn EventBus>),
        );

        // Verify requeue called with 5s delay (first retry)
        let requeue_calls = queue_manager.get_requeue_calls();
        assert_eq!(requeue_calls.len(), 1);
        assert_eq!(requeue_calls[0].0, job_id);
        assert_eq!(requeue_calls[0].1, 5); // First retry: 5s

        // Verify job status is QUEUED (retry)
        let updated_job = job_repo.find_by_id(&job_id).unwrap().unwrap();
        assert_eq!(updated_job.retry_count(), 1);
        assert_eq!(updated_job.status(), &PrintStatus::Queued);
    }

    #[test]
    fn test_non_retryable_error_fails_immediately() {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();
        let arc_conn = Arc::new(StdMutex::new(conn));

        let job_repo = Arc::new(MockPrintJobRepository::new());
        let queue_manager = Arc::new(MockQueueManager::new());
        let event_store = Arc::new(SqliteEventStore::new(arc_conn.clone(), Arc::new(MockSecretManager::new())));
        let event_bus = Arc::new(MockEventBus::new());

        // Create job in QUEUED state
        let mut job = PrintJob::new("https://s3.example.com/doc.pdf".into(), "HP".into());
        let job_id = job.id().clone();
        job.queue().unwrap();
        job_repo.add_job(job.clone());

        // Simulate failure with non-retryable error (validation error)
        let error_message = "Invalid PDF header".to_string();

        // Call handle_job_failure directly (it will mark job as FAILED)
        QueueWorker::handle_job_failure(
            error_message,
            job,
            &(queue_manager.clone() as Arc<dyn QueueManager>),
            &(job_repo.clone() as Arc<dyn PrintJobRepository>),
            &event_store,
            &(event_bus.clone() as Arc<dyn EventBus>),
        );

        // Verify NO requeue call
        let requeue_calls = queue_manager.get_requeue_calls();
        assert_eq!(requeue_calls.len(), 0);

        // Verify job status is FAILED permanently
        let updated_job = job_repo.find_by_id(&job_id).unwrap().unwrap();
        assert_eq!(updated_job.status(), &PrintStatus::Failed);
        assert_eq!(updated_job.retry_count(), 0); // Not incremented
    }

    #[test]
    fn test_max_retries_exceeded_fails_permanently() {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();
        let arc_conn = Arc::new(StdMutex::new(conn));

        let job_repo = Arc::new(MockPrintJobRepository::new());
        let queue_manager = Arc::new(MockQueueManager::new());
        let event_store = Arc::new(SqliteEventStore::new(arc_conn.clone(), Arc::new(MockSecretManager::new())));
        let event_bus = Arc::new(MockEventBus::new());

        // Create job with retry_count already at 3
        let mut job = PrintJob::new("https://s3.example.com/doc.pdf".into(), "HP".into());
        job.queue().unwrap();

        // Simulate 3 previous retries
        job.fail("error 1".into()).unwrap();
        job.retry().unwrap(); // retry_count = 1
        job.fail("error 2".into()).unwrap();
        job.retry().unwrap(); // retry_count = 2
        job.fail("error 3".into()).unwrap();
        job.retry().unwrap(); // retry_count = 3

        let job_id = job.id().clone();
        job_repo.add_job(job.clone());

        // Simulate failure with retryable error (but retries exhausted)
        let error_message = "Timeout after 30s".to_string();

        // Call handle_job_failure directly
        QueueWorker::handle_job_failure(
            error_message,
            job,
            &(queue_manager.clone() as Arc<dyn QueueManager>),
            &(job_repo.clone() as Arc<dyn PrintJobRepository>),
            &event_store,
            &(event_bus.clone() as Arc<dyn EventBus>),
        );

        // Verify NO requeue (max retries exceeded)
        let requeue_calls = queue_manager.get_requeue_calls();
        assert_eq!(requeue_calls.len(), 0);

        // Verify job stays FAILED
        let updated_job = job_repo.find_by_id(&job_id).unwrap().unwrap();
        assert_eq!(updated_job.status(), &PrintStatus::Failed);
        assert_eq!(updated_job.retry_count(), 3);
    }

    #[test]
    fn test_exponential_backoff_progression() {
        use crate::infrastructure::queue::retry_logic::calculate_backoff_delay;

        // Test that subsequent retries use correct delays
        // retry_count = 0 → 5s
        assert_eq!(calculate_backoff_delay(0), 5);
        // retry_count = 1 → 10s
        assert_eq!(calculate_backoff_delay(1), 10);
        // retry_count = 2 → 20s
        assert_eq!(calculate_backoff_delay(2), 20);
    }
}
