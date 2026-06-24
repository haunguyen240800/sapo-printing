use crate::domain::print_job::value_objects::JobId;
use crate::infrastructure::queue::QueueManager;
use std::sync::Arc;

/// Handles PrintJobCreated event by pushing job to the durable queue.
///
/// Called AFTER CreatePrintJobUseCase saves the job.
/// In MVP: called directly from use case (not via async event bus).
pub struct PushToQueueHandler {
    pub queue_manager: Arc<dyn QueueManager>,
}

impl PushToQueueHandler {
    pub fn new(queue_manager: Arc<dyn QueueManager>) -> Self {
        Self { queue_manager }
    }

    /// Push a newly created job to the queue.
    pub fn handle(&self, job_id: &JobId) -> Result<(), String> {
        self.queue_manager
            .push(job_id)
            .map_err(|e| format!("Failed to push job to queue: {}", e))
    }
}
