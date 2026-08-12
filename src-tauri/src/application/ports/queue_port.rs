use crate::domain::print_job::{PrintJob, PrintJobId};

#[derive(Debug)]
pub enum QueueError {
    RepositoryError(String),
    JobNotFound(String),
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

pub trait QueuePort: Send + Sync {
    fn push(&self, job_id: &PrintJobId) -> Result<(), QueueError>;

    fn pop(&self) -> Result<Option<PrintJob>, QueueError>;

    fn requeue(&self, job_id: &PrintJobId, delay_secs: u64) -> Result<(), QueueError>;

    fn queue_depth(&self) -> Result<usize, QueueError>;
}
