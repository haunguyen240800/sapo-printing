//! QueueManager port — application abstraction over the durable print queue.
//!
//! The trait was originally located in `infrastructure::persistence::task_queue`,
//! which forced the application layer to depend on infrastructure. Moving it here
//! lets use cases and handlers depend on `application::ports::QueueManager`
//! without knowing anything about SQLite or any other storage engine.

use crate::domain::print_job::{PrintJob, PrintJobId};

/// Errors specific to queue operations.
#[derive(Debug)]
pub enum QueueError {
    /// Underlying repository failure.
    RepositoryError(String),
    /// Job not found when expected.
    JobNotFound(String),
    /// Job is in wrong state for this operation.
    InvalidState(String),
}

impl std::fmt::Display for QueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RepositoryError(msg) => write!(f, "Queue repository error: {}", msg),
            Self::JobNotFound(id) => write!(f, "Job not found: {}", id),
            Self::InvalidState(msg) => write!(f, "Invalid job state: {}", msg),
        }
    }
}

impl std::error::Error for QueueError {}

/// Contract for the durable print job queue.
pub trait QueueManager: Send + Sync {
    /// Move a PENDING job to QUEUED status.
    /// Returns QueueError::JobNotFound if job doesn't exist.
    /// Returns QueueError::InvalidState if job is not PENDING.
    fn push(&self, job_id: &PrintJobId) -> Result<(), QueueError>;

    /// Pop the oldest QUEUED job and transition it to PENDING (for worker pickup).
    /// Returns None if queue is empty.
    /// Uses SELECT with ORDER BY created_at ASC LIMIT 1.
    fn pop(&self) -> Result<Option<PrintJob>, QueueError>;

    /// Return a job to QUEUED state (e.g., after transient failure).
    /// `delay_secs`: future time offset for exponential backoff.
    fn requeue(&self, job_id: &PrintJobId, delay_secs: u64) -> Result<(), QueueError>;

    /// Return current count of QUEUED jobs.
    fn queue_depth(&self) -> Result<usize, QueueError>;
}
