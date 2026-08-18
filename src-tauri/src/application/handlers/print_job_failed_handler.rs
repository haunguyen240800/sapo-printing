use std::sync::Arc;

use crate::application::errors::Error;
use crate::application::ports::EventStore;
use crate::application::ports::event_bus::EventBus;
use crate::domain::print_job::{PrintJob, PrintJobRepository};

pub struct PrintJobFailedHandler {
    job_repo: Arc<dyn PrintJobRepository>,
    event_store: Arc<dyn EventStore>,
    event_bus: Arc<dyn EventBus>,
}

impl PrintJobFailedHandler {
    pub fn new(
        job_repo: Arc<dyn PrintJobRepository>,
        event_store: Arc<dyn EventStore>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            job_repo,
            event_store,
            event_bus,
        }
    }

    /// Handle a job failure. Marks the job as failed and publishes the failure.
    /// Jobs are never retried — any pipeline error is a permanent failure.
    pub fn handle(&self, error: Error, mut job: PrintJob) {
        let error_msg = error.to_string();
        let error_code = error.code().to_string();

        if let Err(e) = job.fail(error_msg.clone(), error_code) {
            tracing::error!(
                target = "sapo_printer::application::handler::print_job_failed",
                job_id = %job.id(),
                error = ?e,
                "Failed to mark job as failed"
            );
            return;
        }

        tracing::error!(
            target = "sapo_printer::application::handler::print_job_failed",
            job_id = %job.id(),
            reason = %error_msg,
            "Job failed"
        );

        if let Err(e) = self.persist_and_publish(&mut job) {
            tracing::error!(
                target = "sapo_printer::application::handler::print_job_failed",
                job_id = %job.id(),
                error = %e,
                "Failed to persist failed job"
            );
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
                    target = "sapo_printer::application::handler::print_job_failed",
                    event_type = event.event_name(),
                    error = %e,
                    "Failed to publish event (non-fatal)"
                );
            }
        }

        Ok(())
    }
}
