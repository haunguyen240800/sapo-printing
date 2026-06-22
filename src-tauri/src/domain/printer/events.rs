use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use super::value_objects::PrinterId;

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Trait for all domain events emitted by the Printer aggregate.
pub trait PrinterEvent: Send + std::fmt::Debug {
    fn event_type(&self) -> &str;
    fn aggregate_id(&self) -> &PrinterId;
}

/// Fired when a printer transitions to `Online` status.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrinterConnected {
    pub printer_id: PrinterId,
    pub timestamp: u64,
}

impl PrinterConnected {
    pub fn new(printer_id: PrinterId) -> Self {
        Self {
            printer_id,
            timestamp: now_unix(),
        }
    }
}

impl PrinterEvent for PrinterConnected {
    fn event_type(&self) -> &str {
        "PrinterConnected"
    }
    fn aggregate_id(&self) -> &PrinterId {
        &self.printer_id
    }
}

/// Fired when a printer transitions to `Offline` or `Error` status.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrinterDisconnected {
    pub printer_id: PrinterId,
    pub timestamp: u64,
}

impl PrinterDisconnected {
    pub fn new(printer_id: PrinterId) -> Self {
        Self {
            printer_id,
            timestamp: now_unix(),
        }
    }
}

impl PrinterEvent for PrinterDisconnected {
    fn event_type(&self) -> &str {
        "PrinterDisconnected"
    }
    fn aggregate_id(&self) -> &PrinterId {
        &self.printer_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_printer_connected_event_type() {
        let id = PrinterId::new();
        let event = PrinterConnected::new(id.clone());
        assert_eq!(event.event_type(), "PrinterConnected");
        assert_eq!(event.aggregate_id(), &id);
        assert!(event.timestamp > 0);
    }

    #[test]
    fn test_printer_disconnected_event_type() {
        let id = PrinterId::new();
        let event = PrinterDisconnected::new(id.clone());
        assert_eq!(event.event_type(), "PrinterDisconnected");
        assert_eq!(event.aggregate_id(), &id);
        assert!(event.timestamp > 0);
    }

    #[test]
    fn test_events_are_clone() {
        let id = PrinterId::new();
        let e = PrinterConnected::new(id);
        let c = e.clone();
        assert_eq!(c.event_type(), "PrinterConnected");
    }
}
