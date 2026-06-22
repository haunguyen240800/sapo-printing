use std::fmt;
use std::sync::Arc;

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

pub trait EventHandler: Send + Sync {
    fn handle(&self, event_type: &str, payload: &str);
}

pub trait EventBus: Send + Sync {
    fn publish(&self, event_type: &str, payload: &str) -> Result<(), EventBusError>;

    fn subscribe(&self, event_type: &str, handler: Arc<dyn EventHandler>);
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
