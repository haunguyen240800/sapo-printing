use std::fmt;

/// Domain errors for PrintJob aggregate operations.
/// These represent business rule violations — not infrastructure failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DomainError {
    MaxRetryExceeded,
    InvalidStateTransition { from: String, to: String },
    CannotCancelCompleted,
    CannotCancelFailed,
    CannotCancelCancelled,
    RepositoryError { reason: String },
    InvalidStatus { status: String },
    InvalidJobId { raw: String, reason: String },
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MaxRetryExceeded => write!(f, "Maximum retry count (3) exceeded"),
            Self::InvalidStateTransition { from, to } => {
                write!(f, "Invalid state transition from {} to {}", from, to)
            }
            Self::CannotCancelCompleted => write!(f, "Cannot cancel a completed job"),
            Self::CannotCancelFailed => write!(f, "Cannot cancel a failed job"),
            Self::CannotCancelCancelled => write!(f, "Cannot cancel a cancelled job"),
            Self::RepositoryError { reason } => write!(f, "Repository error: {}", reason),
            Self::InvalidStatus { status } => write!(f, "Invalid print status: {}", status),
            Self::InvalidJobId { raw, reason } => {
                write!(f, "Invalid job ID '{}': {}", raw, reason)
            }
        }
    }
}

impl std::error::Error for DomainError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_retry_exceeded_display() {
        let err = DomainError::MaxRetryExceeded;
        assert_eq!(format!("{}", err), "Maximum retry count (3) exceeded");
    }

    #[test]
    fn test_invalid_state_transition_display() {
        let err = DomainError::InvalidStateTransition {
            from: "Pending".to_string(),
            to: "Completed".to_string(),
        };
        assert!(format!("{}", err).contains("Pending"));
        assert!(format!("{}", err).contains("Completed"));
    }

    #[test]
    fn test_cannot_cancel_completed_display() {
        let err = DomainError::CannotCancelCompleted;
        assert!(format!("{}", err).contains("completed"));
    }

    #[test]
    fn test_cannot_cancel_failed_display() {
        let err = DomainError::CannotCancelFailed;
        assert!(format!("{}", err).contains("failed"));
    }

    #[test]
    fn test_cannot_cancel_cancelled_display() {
        let err = DomainError::CannotCancelCancelled;
        assert!(format!("{}", err).contains("cancelled"));
    }

    #[test]
    fn test_domain_error_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<DomainError>();
    }

    #[test]
    fn test_domain_error_clone() {
        let err = DomainError::MaxRetryExceeded;
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }
}
