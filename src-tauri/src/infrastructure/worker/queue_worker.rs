use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::application::errors::Error;
use crate::application::handlers::print_job_failed_handler::PrintJobFailedHandler;
use crate::application::ports::QueuePort;
use crate::application::use_cases::ProcessPrintJobUseCase;
use crate::domain::print_job::PrintJobRepository;

pub struct QueueWorker {
    queue_manager: Arc<dyn QueuePort>,
    job_repo: Arc<dyn PrintJobRepository>,
    process_use_case: Arc<ProcessPrintJobUseCase>,
    failure_handler: Arc<PrintJobFailedHandler>,

    running: Arc<AtomicBool>,
    thread_handle: Mutex<Option<JoinHandle<()>>>,
}

impl QueueWorker {
    pub fn new(
        queue_manager: Arc<dyn QueuePort>,
        job_repo: Arc<dyn PrintJobRepository>,
        process_use_case: Arc<ProcessPrintJobUseCase>,
        failure_handler: Arc<PrintJobFailedHandler>,
    ) -> Self {
        Self {
            queue_manager,
            job_repo,
            process_use_case,
            failure_handler,
            running: Arc::new(AtomicBool::new(false)),
            thread_handle: Mutex::new(None),
        }
    }

    pub fn start(&self) -> Result<(), String> {
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("Worker already running".into());
        }

        let queue_manager = Arc::clone(&self.queue_manager);
        let job_repo = Arc::clone(&self.job_repo);
        let process_use_case = Arc::clone(&self.process_use_case);
        let failure_handler = Arc::clone(&self.failure_handler);
        let running = Arc::clone(&self.running);

        let handle = thread::spawn(move || {
            Self::process_loop(
                queue_manager,
                job_repo,
                process_use_case,
                failure_handler,
                running,
            );
        });

        *self.thread_handle.lock().unwrap_or_else(|p| p.into_inner()) = Some(handle);
        Ok(())
    }

    pub fn stop(&self) -> Result<(), String> {
        self.running.store(false, Ordering::SeqCst);

        let mut handle_guard = self.thread_handle.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(handle) = handle_guard.take() {
            handle.join().map_err(|_| "Failed to join worker thread")?;
        }
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn process_loop(
        queue_manager: Arc<dyn QueuePort>,
        job_repo: Arc<dyn PrintJobRepository>,
        process_use_case: Arc<ProcessPrintJobUseCase>,
        failure_handler: Arc<PrintJobFailedHandler>,
        running: Arc<AtomicBool>,
    ) {
        const POLL_INTERVAL_MS: u64 = 500;

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
                    let job_id = job.id().clone();

                    tracing::info!(
                        target = "sapo_printer::queue_worker",
                        job_id = %job_id,
                        "QueueWorker: picked up job for processing"
                    );

                    // Cô lập panic từng job: một job panic (vd. device context không hợp lệ
                    // sau khi đổi máy in giữa chừng) được chuyển thành lỗi và xử lý qua
                    // failure_handler, thay vì làm chết worker và treo cả batch.
                    let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                        || process_use_case.execute(job),
                    )) {
                        Ok(r) => r,
                        Err(panic_payload) => {
                            let detail = panic_payload
                                .downcast_ref::<&str>()
                                .map(|s| s.to_string())
                                .or_else(|| panic_payload.downcast_ref::<String>().cloned())
                                .unwrap_or_else(|| "unknown panic".to_string());
                            Err(Error::Operation(format!(
                                "Tiến trình in gặp lỗi nghiêm trọng và job này đã bị dừng: {}",
                                detail
                            )))
                        }
                    };

                    if let Err(e) = result {
                        tracing::error!(
                            target = "sapo_printer::queue_worker",
                            job_id = %job_id,
                            error = %e,
                            "QueueWorker: job processing failed"
                        );
                        match job_repo.find_by_id(&job_id) {
                            Ok(Some(failed_job)) => {
                                failure_handler.handle(e, failed_job);
                            }
                            _ => {
                                tracing::error!(
                                    target = "sapo_printer::queue_worker",
                                    job_id = %job_id,
                                    "QueueWorker: Could not load job for failure handling"
                                );
                            }
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
                    thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
                }
                Err(e) => {
                    tracing::error!(
                        target = "sapo_printer::queue_worker",
                        error = %e,
                        "QueueWorker: queue pop failed"
                    );
                    thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
                }
            }
        }
    }
}
