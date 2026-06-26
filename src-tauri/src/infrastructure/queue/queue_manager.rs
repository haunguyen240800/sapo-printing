use crate::domain::print_job::PrintJob;
use crate::domain::print_job::JobId;

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
    fn push(&self, job_id: &JobId) -> Result<(), QueueError>;

    /// Pop the oldest QUEUED job and transition it to PENDING (for worker pickup).
    /// Returns None if queue is empty.
    /// Uses SELECT with ORDER BY created_at ASC LIMIT 1.
    fn pop(&self) -> Result<Option<PrintJob>, QueueError>;

    /// Return a job to QUEUED state (e.g., after transient failure).
    /// `delay_secs`: future time offset (for exponential backoff in Story 3.6).
    /// For this story: ignore delay_secs, just set status back to QUEUED.
    fn requeue(&self, job_id: &JobId, delay_secs: u64) -> Result<(), QueueError>;

    /// Return current count of QUEUED jobs.
    fn queue_depth(&self) -> Result<usize, QueueError>;
}
