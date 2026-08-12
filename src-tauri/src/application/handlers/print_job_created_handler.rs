use crate::application::ports::QueuePort;
use crate::application::ports::event_bus::EventHandler;
use crate::domain::print_job::events::PrintJobCreated;
use std::sync::Arc;

pub struct PrintJobCreatedHandler {
    pub queue_manager: Arc<dyn QueuePort>,
}

impl PrintJobCreatedHandler {
    pub fn new(queue_manager: Arc<dyn QueuePort>) -> Self {
        Self { queue_manager }
    }
}

impl EventHandler for PrintJobCreatedHandler {
    fn handle(&self, event_type: &str, payload: &str) {
        tracing::debug!(
            target = "sapo_printer::application::handler::print_job_created",
            event_type = event_type,
            "PrintJobCreatedHandler: received event"
        );

        if event_type != "PrintJobCreated" {
            return;
        }

        let event: PrintJobCreated = match serde_json::from_str(payload) {
            Ok(e) => e,
            Err(e) => {
                tracing::error!(
                    target = "sapo_printer::application::handler::print_job_created",
                    error = %e,
                    payload = payload,
                    "Failed to deserialize PrintJobCreated payload"
                );
                return;
            }
        };

        if let Err(e) = self.queue_manager.push(&event.job_id) {
            tracing::error!(
                target = "sapo_printer::application::handler::print_job_created",
                job_id = %event.job_id,
                error = %e,
                "Failed to push job to queue"
            );
        } else {
            tracing::info!(
                target = "sapo_printer::application::handler::print_job_created",
                job_id = %event.job_id,
                "Job pushed to queue"
            );
        }
    }
}
