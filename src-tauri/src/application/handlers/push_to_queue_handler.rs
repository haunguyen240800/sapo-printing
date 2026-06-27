use crate::domain::models::JobId;
use crate::infrastructure::persistence::task_queue::QueueManager;
use crate::shared::event_bus::EventHandler;
use std::str::FromStr;
use std::sync::Arc;

/// Handles PrintJobCreated event by pushing job to the durable queue.
///
/// Subscribes to "PrintJobCreated" event via EventBus.
/// Implements EventHandler trait for event-driven architecture.
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
        tracing::info!(
            target = "sapo_printer::handlers::push_to_queue",
            event_type = event_type,
            payload = payload,
            "PushToQueueHandler: received event"
        );

        if event_type != "PrintJobCreated" {
            tracing::debug!(
                target = "sapo_printer::handlers::push_to_queue",
                event_type = event_type,
                "PushToQueueHandler: ignoring non-PrintJobCreated event"
            );
            return; // Ignore other events
        }

        // Parse job_id from JSON payload
        let payload_json: serde_json::Value = match serde_json::from_str(payload) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(
                    target = "sapo_printer::handlers::push_to_queue",
                    error = %e,
                    "Failed to parse event payload"
                );
                return;
            }
        };

        let job_id_str = match payload_json["job_id"].as_str() {
            Some(id) => id,
            None => {
                tracing::error!(
                    target = "sapo_printer::handlers::push_to_queue",
                    "Missing job_id in PrintJobCreated event payload"
                );
                return;
            }
        };

        let job_id = match JobId::from_str(job_id_str) {
            Ok(id) => id,
            Err(e) => {
                tracing::error!(
                    target = "sapo_printer::handlers::push_to_queue",
                    job_id = job_id_str,
                    error = %e,
                    "Invalid job_id format"
                );
                return;
            }
        };

        // Push to queue
        if let Err(e) = self.queue_manager.push(&job_id) {
            tracing::error!(
                target = "sapo_printer::handlers::push_to_queue",
                job_id = %job_id,
                error = %e,
                "Failed to push job to queue"
            );
        } else {
            tracing::info!(
                target = "sapo_printer::handlers::push_to_queue",
                job_id = %job_id,
                "Job pushed to queue successfully"
            );
        }
    }
}
