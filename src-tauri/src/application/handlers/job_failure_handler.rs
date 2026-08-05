use std::sync::Arc;

use crate::application::errors::PipelineError;
use crate::application::policies::retry_policy::calculate_backoff_delay;
use crate::application::ports::{EventStore, QueueManager};
use crate::domain::print_job::{PrintJob, PrintJobRepository, MAX_RETRY_COUNT};
use crate::shared::event_bus::EventBus;

pub struct JobFailureHandler {
    queue_manager: Arc<dyn QueueManager>,
    job_repo: Arc<dyn PrintJobRepository>,
    event_store: Arc<dyn EventStore>,
    event_bus: Arc<dyn EventBus>,
}

impl JobFailureHandler {
    pub fn new(
        queue_manager: Arc<dyn QueueManager>,
        job_repo: Arc<dyn PrintJobRepository>,
        event_store: Arc<dyn EventStore>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            queue_manager,
            job_repo,
            event_store,
            event_bus,
        }
    }

    /// Handle a job failure. Marks the job as failed and either requeues it
    /// with backoff (retryable) or persists the permanent failure.
    pub fn handle(&self, error: PipelineError, mut job: PrintJob) {
        let error_msg = error.to_string();
        let retryable = error.is_retryable();

        if let Err(e) = job.fail(error_msg.clone()) {
            tracing::error!(
                target = "sapo_printer::application::handler::job_failure",
                job_id = %job.id(),
                error = ?e,
                "Failed to mark job as failed"
            );
            return;
        }

        let can_retry = retryable && job.retry_count() < MAX_RETRY_COUNT;

        if can_retry {
            let delay = calculate_backoff_delay(job.retry_count());

            match job.retry() {
                Ok(()) => {
                    tracing::warn!(
                        target = "sapo_printer::application::handler::job_failure",
                        job_id = %job.id(),
                        retry_count = job.retry_count(),
                        delay_secs = delay,
                        error = %error_msg,
                        "Job failed, retrying"
                    );

                    if let Err(e) = self.queue_manager.requeue(job.id(), delay) {
                        tracing::error!(
                            target = "sapo_printer::application::handler::job_failure",
                            job_id = %job.id(),
                            error = %e,
                            "Failed to requeue job"
                        );
                        let _ = job.fail(format!("Requeue failed: {}", e));
                        let _ = self.persist_and_publish(&mut job);
                        return;
                    }

                    if let Err(e) = self.persist_and_publish(&mut job) {
                        tracing::error!(
                            target = "sapo_printer::application::handler::job_failure",
                            job_id = %job.id(),
                            error = %e,
                            "Failed to persist retry"
                        );
                    }
                }
                Err(e) => {
                    tracing::error!(
                        target = "sapo_printer::application::handler::job_failure",
                        job_id = %job.id(),
                        retry_count = job.retry_count(),
                        error = ?e,
                        "Cannot retry job"
                    );
                    for attempt in 0..2 {
                        match self.persist_and_publish(&mut job) {
                            Ok(()) => break,
                            Err(e) if attempt == 0 => {
                                tracing::warn!(
                                    target = "sapo_printer::application::handler::job_failure",
                                    job_id = %job.id(),
                                    error = %e,
                                    "Persist failed, retrying once"
                                );
                                std::thread::sleep(std::time::Duration::from_millis(100));
                            }
                            Err(e) => {
                                tracing::error!(
                                    target = "sapo_printer::application::handler::job_failure",
                                    job_id = %job.id(),
                                    error = %e,
                                    "Failed to persist failed job after retry"
                                );
                            }
                        }
                    }
                }
            }
        } else {
            let reason = if job.retry_count() >= MAX_RETRY_COUNT {
                format!("Max retries exceeded ({}): {}", MAX_RETRY_COUNT, error_msg)
            } else {
                format!("Non-retryable error: {}", error_msg)
            };

            tracing::error!(
                target = "sapo_printer::application::handler::job_failure",
                job_id = %job.id(),
                reason = %reason,
                "Job failed permanently"
            );

            if let Err(e) = self.persist_and_publish(&mut job) {
                tracing::error!(
                    target = "sapo_printer::application::handler::job_failure",
                    job_id = %job.id(),
                    error = %e,
                    "Failed to persist failed job"
                );
            }
        }
    }

    fn persist_and_publish(&self, job: &mut PrintJob) -> Result<(), String> {
        let events = job.drain_events();

        self.event_store
            .save_all(job.id().to_string().as_str(), &events)
            .map_err(|e| format!("Failed to save events: {:?}", e))?;

        self.job_repo
            .update(job)
            .map_err(|e| format!("Failed to update job: {:?}", e))?;

        for event in &events {
            let payload = event.serialize_payload();
            if let Err(e) = self.event_bus.publish(event.event_name(), &payload) {
                tracing::warn!(
                    target = "sapo_printer::application::handler::job_failure",
                    event_type = event.event_name(),
                    error = %e,
                    "Failed to publish event (non-fatal)"
                );
            }
        }

        Ok(())
    }
}
