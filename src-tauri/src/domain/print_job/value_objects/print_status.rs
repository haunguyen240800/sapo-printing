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

    /// Serialize to the canonical UPPER_CASE database representation.
    ///
    /// This is the single source of truth for status string serialization shared
    /// by all SQLite adapters (`SqlitePrintJobRepository`, `SqliteQueueManager`,
    /// `MetricsCollector`). Changing this requires updating the DB schema default.
    pub fn to_db_string(&self) -> &'static str {
        match self {
            PrintStatus::Pending => "PENDING",
            PrintStatus::Queued => "QUEUED",
            PrintStatus::Downloaded => "DOWNLOADED",
            PrintStatus::SubmittedToQueue => "SUBMITTED_TO_QUEUE",
            PrintStatus::Printing => "PRINTING",
            PrintStatus::Completed => "COMPLETED",
            PrintStatus::Failed => "FAILED",
            PrintStatus::Cancelled => "CANCELLED",
        }
    }

    /// Parse from the canonical UPPER_CASE database representation.
    ///
    /// Returns `Err(invalid_value)` if the string does not match any variant.
    pub fn from_db_string(s: &str) -> Result<Self, String> {
        match s {
            "PENDING" => Ok(PrintStatus::Pending),
            "QUEUED" => Ok(PrintStatus::Queued),
            "DOWNLOADED" => Ok(PrintStatus::Downloaded),
            "SUBMITTED_TO_QUEUE" => Ok(PrintStatus::SubmittedToQueue),
            "PRINTING" => Ok(PrintStatus::Printing),
            "COMPLETED" => Ok(PrintStatus::Completed),
            "FAILED" => Ok(PrintStatus::Failed),
            "CANCELLED" => Ok(PrintStatus::Cancelled),
            _ => Err(s.to_string()),
        }
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
    fn test_to_db_string_uses_upper_case() {
        assert_eq!(PrintStatus::Pending.to_db_string(), "PENDING");
        assert_eq!(PrintStatus::Queued.to_db_string(), "QUEUED");
        assert_eq!(PrintStatus::SubmittedToQueue.to_db_string(), "SUBMITTED_TO_QUEUE");
        assert_eq!(PrintStatus::Failed.to_db_string(), "FAILED");
    }

    #[test]
    fn test_from_db_string_roundtrip() {
        for status in [
            PrintStatus::Pending,
            PrintStatus::Queued,
            PrintStatus::Downloaded,
            PrintStatus::SubmittedToQueue,
            PrintStatus::Printing,
            PrintStatus::Completed,
            PrintStatus::Failed,
            PrintStatus::Cancelled,
        ] {
            let s = status.to_db_string();
            assert_eq!(PrintStatus::from_db_string(s).unwrap(), status);
        }
    }

    #[test]
    fn test_from_db_string_rejects_unknown() {
        assert!(PrintStatus::from_db_string("Pending").is_err());
        assert!(PrintStatus::from_db_string("unknown").is_err());
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
