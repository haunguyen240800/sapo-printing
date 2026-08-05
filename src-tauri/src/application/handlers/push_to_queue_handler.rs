use crate::application::ports::QueueManager;
use crate::domain::print_job::events::PrintJobCreated;
use crate::shared::event_bus::EventHandler;
use std::sync::Arc;

pub struct PushToQueueHandler {
    pub queue_manager: Arc<dyn QueueManager>,
}

impl PushToQueueHandler {
    pub fn new(queue_manager: Arc<dyn QueueManager>) -> Self {
        Self { queue_manager }
    }
}

impl EventHandler for PushToQueueHandler {
    fn handle(&self, event_type: &str, payload: &str) {
        tracing::debug!(
            target = "sapo_printer::application::handler::push_to_queue",
            event_type = event_type,
            "PushToQueueHandler: received event"
        );

        if event_type != "PrintJobCreated" {
            return;
        }

        let event: PrintJobCreated = match serde_json::from_str(payload) {
            Ok(e) => e,
            Err(e) => {
                tracing::error!(
                    target = "sapo_printer::application::handler::push_to_queue",
                    error = %e,
                    payload = payload,
                    "Failed to deserialize PrintJobCreated payload"
                );
                return;
            }
        };

        if let Err(e) = self.queue_manager.push(&event.job_id) {
            tracing::error!(
                target = "sapo_printer::application::handler::push_to_queue",
                job_id = %event.job_id,
                error = %e,
                "Failed to push job to queue"
            );
        } else {
            tracing::info!(
                target = "sapo_printer::application::handler::push_to_queue",
                job_id = %event.job_id,
                "Job pushed to queue"
            );
        }
    }
}
