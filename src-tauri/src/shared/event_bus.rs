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

/// In-memory event bus — logs events but does not persist them.
/// Future: replace with outbox-backed persistent queue.
pub struct InMemoryEventBus;

impl InMemoryEventBus {
    pub fn new() -> Self {
        Self
    }
}

impl Default for InMemoryEventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus for InMemoryEventBus {
    fn publish(&self, event_type: &str, _payload: &str) -> Result<(), EventBusError> {
        tracing::debug!(
            target = "sapo_printer::event_bus",
            event_type = event_type,
            bus = "in_memory",
            "EventBus: event published"
        );
        // No-op for now — events are persisted via EventStore in the same transaction.
        // The outbox worker will pick them up later.
        Ok(())
    }
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

    #[test]
    fn test_in_memory_event_bus_publish_returns_ok() {
        let bus = InMemoryEventBus::new();
        assert!(bus.publish("TestEvent", "{}").is_ok());
    }

    #[test]
    fn test_in_memory_event_bus_default() {
        let bus = InMemoryEventBus::default();
        assert!(bus.publish("AnyEvent", "{\"key\":\"value\"}").is_ok());
    }
}
