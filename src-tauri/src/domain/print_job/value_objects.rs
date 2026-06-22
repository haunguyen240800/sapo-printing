use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

/// Unique identifier for a print job, backed by UUID v4.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(Uuid);

impl JobId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for JobId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

/// Print job lifecycle states.
///
/// State machine:
/// ```text
/// PENDING → QUEUED → DOWNLOADED → SUBMITTED_TO_QUEUE → PRINTING → COMPLETED
///                ↓                ↓                      ↓
///             FAILED  ←───────── FAILED  ←──────────── FAILED
///                ↓
///             QUEUED (retry, if retry_count < 3)
///
/// Any non-terminal state → CANCELLED (via cancel())
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrintStatus {
    Pending,
    Queued,
    Downloaded,
    SubmittedToQueue,
    Printing,
    Completed,
    Failed,
    Cancelled,
}

impl PrintStatus {
    /// Returns true for terminal states (no further transitions allowed).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            PrintStatus::Completed | PrintStatus::Failed | PrintStatus::Cancelled
        )
    }

    /// Checks whether a transition to `target` is valid from this state.
    pub fn can_transition_to(&self, target: &PrintStatus) -> bool {
        matches!(
            (self, target),
            (PrintStatus::Pending, PrintStatus::Queued)
                | (PrintStatus::Queued, PrintStatus::Downloaded)
                | (PrintStatus::Queued, PrintStatus::Failed)
                | (PrintStatus::Downloaded, PrintStatus::SubmittedToQueue)
                | (PrintStatus::SubmittedToQueue, PrintStatus::Printing)
                | (PrintStatus::SubmittedToQueue, PrintStatus::Failed)
                | (PrintStatus::Printing, PrintStatus::Completed)
                | (PrintStatus::Printing, PrintStatus::Failed)
                | (PrintStatus::Failed, PrintStatus::Queued)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_id_unique() {
        let id1 = JobId::new();
        let id2 = JobId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_job_id_to_string() {
        let id = JobId::new();
        let s = id.to_string();
        assert_eq!(s.len(), 36); // UUID format: 8-4-4-4-12
        assert!(s.contains('-'));
    }

    #[test]
    fn test_job_id_from_str() {
        let id = JobId::new();
        let s = id.to_string();
        let parsed: JobId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_job_id_from_str_invalid() {
        let result: Result<JobId, _> = "not-a-uuid".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_job_id_clone_eq() {
        let id = JobId::new();
        let cloned = id.clone();
        assert_eq!(id, cloned);
    }

    #[test]
    fn test_pending_is_not_terminal() {
        assert!(!PrintStatus::Pending.is_terminal());
    }

    #[test]
    fn test_queued_is_not_terminal() {
        assert!(!PrintStatus::Queued.is_terminal());
    }

    #[test]
    fn test_completed_is_terminal() {
        assert!(PrintStatus::Completed.is_terminal());
    }

    #[test]
    fn test_failed_is_terminal() {
        assert!(PrintStatus::Failed.is_terminal());
    }

    #[test]
    fn test_cancelled_is_terminal() {
        assert!(PrintStatus::Cancelled.is_terminal());
    }

    #[test]
    fn test_valid_transitions() {
        assert!(PrintStatus::Pending.can_transition_to(&PrintStatus::Queued));
        assert!(PrintStatus::Queued.can_transition_to(&PrintStatus::Downloaded));
        assert!(PrintStatus::Queued.can_transition_to(&PrintStatus::Failed));
        assert!(PrintStatus::Downloaded.can_transition_to(&PrintStatus::SubmittedToQueue));
        assert!(PrintStatus::SubmittedToQueue.can_transition_to(&PrintStatus::Printing));
        assert!(PrintStatus::SubmittedToQueue.can_transition_to(&PrintStatus::Failed));
        assert!(PrintStatus::Printing.can_transition_to(&PrintStatus::Completed));
        assert!(PrintStatus::Printing.can_transition_to(&PrintStatus::Failed));
        assert!(PrintStatus::Failed.can_transition_to(&PrintStatus::Queued));
    }

    #[test]
    fn test_invalid_transitions() {
        assert!(!PrintStatus::Pending.can_transition_to(&PrintStatus::Completed));
        assert!(!PrintStatus::Pending.can_transition_to(&PrintStatus::Printing));
        assert!(!PrintStatus::Completed.can_transition_to(&PrintStatus::Pending));
        assert!(!PrintStatus::Failed.can_transition_to(&PrintStatus::Completed));
        assert!(!PrintStatus::Downloaded.can_transition_to(&PrintStatus::Queued));
        assert!(!PrintStatus::Printing.can_transition_to(&PrintStatus::Queued));
        assert!(!PrintStatus::Cancelled.can_transition_to(&PrintStatus::Queued));
        assert!(!PrintStatus::Cancelled.can_transition_to(&PrintStatus::Failed));
    }
}
