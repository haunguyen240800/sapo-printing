//! Queue Worker â€” Background Job Processor
//!
//! Implements a background worker thread that continuously polls the queue
//! and processes print jobs through the complete pipeline:
//! pop â†’ download â†’ render â†’ print â†’ complete.
//!
//! ## Architecture
//! - **Pattern:** Worker Pattern + Background Processing
//! - **Layer:** Infrastructure (Queue Management)
//! - **Thread Safety:** All dependencies wrapped in Arc, AtomicBool for lifecycle control
//! - **Cleanup:** RAII via TempPdfFile Drop trait (automatic temp file cleanup)
//!
//! ## Lifecycle
//! 1. `start()` â€” spawns background thread, begins polling
//! 2. `process_loop()` â€” polls queue every 500ms, processes jobs sequentially
//! 3. `stop()` â€” sets running=false, waits for graceful shutdown
//!
//! ## Event-Driven Architecture
//! Worker publishes 5 events per job:
//! - PrintJobQueued (PENDING â†’ QUEUED)
//! - PrintJobDownloaded (QUEUED â†’ DOWNLOADED)
//! - PrintJobSubmitted (DOWNLOADED â†’ SUBMITTED_TO_QUEUE)
//! - PrintJobPrinting (SUBMITTED_TO_QUEUE â†’ PRINTING)
//! - PrintJobCompleted (PRINTING â†’ COMPLETED)
//!
//! ## Story Context
//! This is Story 3.5 â€” implements sequential job processing only.
//! - âœ… Single worker thread
//! - âœ… FIFO queue processing
//! - âœ… State transitions + event publishing
//! - âŒ Auto-retry logic (Story 3.6)
//! - âŒ Job cancellation (Story 3.7)
//! - âŒ Concurrent workers (Story 4.x)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::domain::print_job::PrintJob;
use crate::domain::print_job::PrintJobRepository;
use crate::infrastructure::database::SqliteEventStore;
use crate::infrastructure::downloader::DocumentDownloader;
use crate::infrastructure::queue::QueueManager;
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
    ) -> Self {
        Self {
            queue_manager,
            job_repo,
            event_store,
            event_bus,
            downloader,
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
        let running = Arc::clone(&self.running);

        // Spawn background thread
        let handle = thread::spawn(move || {
            Self::process_loop(
                queue_manager,
                job_repo,
                event_store,
                event_bus,
                downloader,
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

    /// Main processing loop â€” runs in background thread.
    ///
    /// Polls the queue every 500ms, processes jobs sequentially, and handles errors gracefully.
    #[allow(clippy::too_many_arguments)]
    fn process_loop(
        queue_manager: Arc<dyn QueueManager>,
        job_repo: Arc<dyn PrintJobRepository>,
        event_store: Arc<SqliteEventStore>,
        event_bus: Arc<dyn EventBus>,
        downloader: Arc<dyn DocumentDownloader>,
        running: Arc<AtomicBool>,
    ) {
        const POLL_INTERVAL_MS: u64 = 500; // 0.5s poll interval

        tracing::info!(
            target = "sapo_printer::queue_worker",
            "QueueWorker: processing loop started"
        );

        let mut poll_count = 0;
        while running.load(Ordering::SeqCst) {
            poll_count += 1;
            if poll_count % 10 == 0 {
                tracing::debug!(
                    target = "sapo_printer::queue_worker",
                    poll_count = poll_count,
                    "QueueWorker: still running, polling queue"
                );
            }

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
    /// 1. PENDING â†’ QUEUED (transition from pop state)
    /// 2. Download document
    /// 3. QUEUED â†’ DOWNLOADED
    /// 4. Render document
    /// 5. DOWNLOADED â†’ SUBMITTED_TO_QUEUE
    /// 6. SUBMITTED_TO_QUEUE â†’ PRINTING
    /// 7. Send to printer
    /// 8. PRINTING â†’ COMPLETED
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
    ) -> Result<(), String> {
        // Step 1: Transition to Queued (pop returns Pending, we transition to Queued)
        job.queue()
            .map_err(|e| format!("Failed to queue job: {:?}", e))?;
        if let Err(e) = Self::persist_and_publish(&mut job, job_repo, event_store, event_bus) {
            // Persist failed â€” job is stuck in Pending in DB.
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

        // Step 3: Render document & Step 4: Send to printer
        let render_start = std::time::Instant::now();
        
        let mut backend = crate::infrastructure::graphics::backend::GraphicsBackendFactory::create();
        backend.begin_document(job.printer_name(), "Sapo Print Job").map_err(|e| format!("Begin document failed: {}", e))?;
        
        let strategy: Box<dyn crate::infrastructure::pdfium::renderer::RenderStrategy> = if job.settings.print_as_image {
            Box::new(crate::infrastructure::pdfium::bitmap_strategy::BitmapRenderStrategy::new())
        } else {
            Box::new(crate::infrastructure::pdfium::native_strategy::NativePdfRenderStrategy::new())
        };
        
        strategy.render(
            temp_file.path().to_str().unwrap(), 
            &job.settings, 
            &mut *backend
        );
        backend.end_document();
        
        let render_duration = render_start.elapsed();
        tracing::info!(
            target = "sapo_printer::metrics",
            job_id = %job.id(),
            step = "render_and_print",
            duration_ms = render_duration.as_millis() as u64,
            "Pipeline step completed"
        );

        job.mark_submitted().map_err(|e| format!("Failed to mark submitted: {:?}", e))?;
        Self::persist_and_publish(&mut job, job_repo, event_store, event_bus)?;
        
        job.mark_printing().map_err(|e| format!("Failed to mark printing: {:?}", e))?;
        Self::persist_and_publish(&mut job, job_repo, event_store, event_bus)?;

        // Step 5: Mark complete
        job.complete()
            .map_err(|e| format!("Failed to mark complete: {:?}", e))?;
        Self::persist_and_publish(&mut job, job_repo, event_store, event_bus)?;

        // Step 6: temp_file is dropped here â†’ RAII cleanup deletes the file

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
        // If this fails, job state is not updated â€” consistent (no change).
        // Events are drained but the job stays in its old state and can be retried.
        event_store
            .save_all(job.id().to_string().as_str(), &events)
            .map_err(|e| format!("Failed to save events: {:?}", e))?;

        // 3. Persist job state
        // If this fails after events were saved, events exist in store but job
        // state is not updated â€” recoverable via event replay.
        job_repo
            .update(job)
            .map_err(|e| format!("Failed to update job: {:?}", e))?;

        // 4. Publish events to event bus (non-fatal)
        for event in &events {
            let payload = event.serialize_payload();
            if let Err(e) = event_bus.publish(event.event_name(), &payload) {
                eprintln!(
                    "Warning: Failed to publish event {}: {}",
                    event.event_name(),
                    e
                );
                // Non-fatal â€” continue processing
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
    ///    - Call job.retry() (increments retry_count, FAILED â†’ QUEUED)
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


