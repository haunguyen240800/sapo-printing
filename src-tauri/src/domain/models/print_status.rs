use serde::{Deserialize, Serialize};

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
    ///
    /// Valid transitions follow the job processing pipeline with failure exits:
    /// - Forward progression: Pending → Queued → Downloaded → SubmittedToQueue → Printing → Completed
    /// - Failure exits: Any intermediate state can transition to Failed
    /// - Retry: Failed → Queued (for auto-retry logic)
    pub fn can_transition_to(&self, target: &PrintStatus) -> bool {
        matches!(
            (self, target),
            (PrintStatus::Pending, PrintStatus::Queued)
                | (PrintStatus::Queued, PrintStatus::Downloaded)
                | (PrintStatus::Queued, PrintStatus::Failed)
                | (PrintStatus::Downloaded, PrintStatus::SubmittedToQueue)
                | (PrintStatus::Downloaded, PrintStatus::Failed) // Render failure
                | (PrintStatus::SubmittedToQueue, PrintStatus::Printing)
                | (PrintStatus::SubmittedToQueue, PrintStatus::Failed)
                | (PrintStatus::Printing, PrintStatus::Completed)
                | (PrintStatus::Printing, PrintStatus::Failed)
                | (PrintStatus::Failed, PrintStatus::Queued) // Retry transition
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(PrintStatus::Downloaded.can_transition_to(&PrintStatus::Failed));
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
