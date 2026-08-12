use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::application::ports::event_bus::{EventBus, EventBusError, EventHandler};

/// In-memory, synchronous `EventBus` implementation.
///
/// Handlers are invoked inline on the publishing thread. Suitable for the
/// single-process desktop app; a persistent outbox-backed bus can replace it
/// later without touching publishers (they depend on the `EventBus` port).
pub struct InMemoryEventBus {
    handlers: Mutex<HashMap<String, Vec<Arc<dyn EventHandler>>>>,
}

impl InMemoryEventBus {
    pub fn new() -> Self {
        Self {
            handlers: Mutex::new(HashMap::new()),
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

        // Invoke all registered handlers for this event type.
        let handlers = self.handlers.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(handler_list) = handlers.get(event_type) {
            for handler in handler_list {
                handler.handle(event_type, payload);
            }
        }

        Ok(())
    }

    fn subscribe(&self, event_type: &str, handler: Arc<dyn EventHandler>) {
        let mut handlers = self.handlers.lock().unwrap_or_else(|p| p.into_inner());
        handlers
            .entry(event_type.to_string())
            .or_default()
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
