use std::fmt;

/// Errors that can occur when publishing events.
#[derive(Debug)]
pub enum EventBusError {
    PublishFailed { reason: String },
}

impl fmt::Display for EventBusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EventBusError::PublishFailed { reason } => {
                write!(f, "Event publish failed: {}", reason)
            }
        }
    }
}

impl std::error::Error for EventBusError {}

/// Contract for publishing domain events.
///
/// Implementations live in the Infrastructure layer (`InMemoryEventBus`, etc.).
/// Both `event_type` and `payload` are plain strings for maximum flexibility
/// until the event schema stabilises in Epic 2+.
pub trait EventBus: Send + Sync {
    fn publish(&self, event_type: &str, payload: &str) -> Result<(), EventBusError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_bus_error_display_contains_reason() {
        let err = EventBusError::PublishFailed {
            reason: "connection refused".to_string(),
        };
        assert!(err.to_string().contains("connection refused"));
    }
}
