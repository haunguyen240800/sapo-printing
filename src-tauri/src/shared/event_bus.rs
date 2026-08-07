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

pub struct InMemoryEventBus {
    handlers: std::sync::Mutex<std::collections::HashMap<String, Vec<Arc<dyn EventHandler>>>>,
}

impl InMemoryEventBus {
    pub fn new() -> Self {
        Self {
            handlers: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

impl Default for InMemoryEventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl EventBus for InMemoryEventBus {
    fn publish(&self, event_type: &str, payload: &str) -> Result<(), EventBusError> {
        tracing::debug!(
            target = "sapo_printer::event_bus",
            event_type = event_type,
            bus = "in_memory",
            "EventBus: event published"
        );

        // Invoke all registered handlers for this event type
        let handlers = self.handlers.lock().unwrap();
        if let Some(handler_list) = handlers.get(event_type) {
            for handler in handler_list {
                handler.handle(event_type, payload);
            }
        }

        Ok(())
    }

    fn subscribe(&self, event_type: &str, handler: Arc<dyn EventHandler>) {
        let mut handlers = self.handlers.lock().unwrap();
        handlers
            .entry(event_type.to_string())
            .or_insert_with(Vec::new)
            .push(handler);

        tracing::debug!(
            target = "sapo_printer::event_bus",
            event_type = event_type,
            "EventBus: handler subscribed"
        );
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
