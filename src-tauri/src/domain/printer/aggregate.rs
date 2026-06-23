use serde::{Deserialize, Serialize};

use super::errors::PrinterDomainError;
use super::events::{PrinterConnected, PrinterDisconnected, PrinterEvent};
use super::value_objects::{PrinterId, PrinterName, PrinterStatus, PrinterType};

/// Printer aggregate root.
///
/// Encapsulates printer lifecycle: tracks identity, current availability status,
/// and collects domain events for the Outbox pattern (drain after persistence).
#[derive(Debug, Serialize, Deserialize)]
pub struct Printer {
    id: PrinterId,
    name: PrinterName,
    status: PrinterStatus,
    printer_type: PrinterType,
    /// Transient event buffer — skipped during serialization; drain after persistence.
    #[serde(skip)]
    events: Vec<Box<dyn PrinterEvent>>,
}

/// Manual Clone: events buffer is transient — clone produces empty vec.
impl Clone for Printer {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            name: self.name.clone(),
            status: self.status.clone(),
            printer_type: self.printer_type.clone(),
            events: vec![],
        }
    }
}

impl Printer {
    /// Create a new printer. Initial status is `Offline`. No event emitted.
    pub fn new(name: PrinterName, printer_type: PrinterType) -> Self {
        Self {
            id: PrinterId::new(),
            name,
            status: PrinterStatus::Offline,
            printer_type,
            events: vec![],
        }
    }

    // ── Accessors ──────────────────────────────────────────────────────────

    pub fn id(&self) -> &PrinterId {
        &self.id
    }

    pub fn name(&self) -> &PrinterName {
        &self.name
    }

    pub fn status(&self) -> &PrinterStatus {
        &self.status
    }

    pub fn printer_type(&self) -> &PrinterType {
        &self.printer_type
    }

    // ── Business Rules ─────────────────────────────────────────────────────

    /// Returns `true` only when printer is `Online`.
    /// Must be checked before assigning a print job.
    pub fn can_accept_job(&self) -> bool {
        self.status == PrinterStatus::Online
    }

    // ── State Transitions ──────────────────────────────────────────────────

    /// Transition to `Online`. Emits `PrinterConnected`. No-op if already `Online`.
    pub fn connect(&mut self) -> Result<(), PrinterDomainError> {
        if self.status == PrinterStatus::Online {
            return Ok(());
        }
        self.status = PrinterStatus::Online;
        self.events
            .push(Box::new(PrinterConnected::new(self.id.clone())));
        Ok(())
    }

    /// Transition to `Offline`. Emits `PrinterDisconnected`. No-op if already `Offline`.
    pub fn disconnect(&mut self) -> Result<(), PrinterDomainError> {
        if self.status == PrinterStatus::Offline {
            return Ok(());
        }
        self.status = PrinterStatus::Offline;
        self.events
            .push(Box::new(PrinterDisconnected::new(self.id.clone())));
        Ok(())
    }

    /// Transition to `Error`. Emits `PrinterDisconnected` (printer no longer available). No-op if already `Error`.
    pub fn set_error(&mut self) -> Result<(), PrinterDomainError> {
        if self.status == PrinterStatus::Error {
            return Ok(());
        }
        self.status = PrinterStatus::Error;
        self.events
            .push(Box::new(PrinterDisconnected::new(self.id.clone())));
        Ok(())
    }

    // ── Event Buffer ───────────────────────────────────────────────────────

    /// Drain and return all collected events. Clears the internal buffer.
    /// Call after persisting the aggregate (Outbox pattern).
    pub fn drain_events(&mut self) -> Vec<Box<dyn PrinterEvent>> {
        std::mem::take(&mut self.events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_printer() -> Printer {
        Printer::new(
            PrinterName::new("HP LaserJet".to_string()),
            PrinterType::Local,
        )
    }

    #[test]
    fn test_printer_new_status_is_offline() {
        let p = make_printer();
        assert_eq!(p.status(), &PrinterStatus::Offline);
    }

    #[test]
    fn test_printer_new_no_events() {
        let mut p = make_printer();
        assert!(p.drain_events().is_empty());
    }

    #[test]
    fn test_connect_transitions_to_online() {
        let mut p = make_printer();
        p.connect().unwrap();
        assert_eq!(p.status(), &PrinterStatus::Online);
    }

    #[test]
    fn test_connect_emits_connected_event() {
        let mut p = make_printer();
        p.connect().unwrap();
        let events = p.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type(), "PrinterConnected");
    }

    #[test]
    fn test_disconnect_transitions_to_offline() {
        let mut p = make_printer();
        p.connect().unwrap();
        let _ = p.drain_events();
        p.disconnect().unwrap();
        assert_eq!(p.status(), &PrinterStatus::Offline);
    }

    #[test]
    fn test_disconnect_emits_disconnected_event() {
        let mut p = make_printer();
        p.connect().unwrap();
        let _ = p.drain_events();
        p.disconnect().unwrap();
        let events = p.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type(), "PrinterDisconnected");
    }

    #[test]
    fn test_set_error_transitions_to_error() {
        let mut p = make_printer();
        p.set_error().unwrap();
        assert_eq!(p.status(), &PrinterStatus::Error);
    }

    #[test]
    fn test_set_error_emits_disconnected_event() {
        let mut p = make_printer();
        p.set_error().unwrap();
        let events = p.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type(), "PrinterDisconnected");
    }

    #[test]
    fn test_can_accept_job_online() {
        let mut p = make_printer();
        p.connect().unwrap();
        assert!(p.can_accept_job());
    }

    #[test]
    fn test_cannot_accept_job_offline() {
        let p = make_printer(); // starts Offline
        assert!(!p.can_accept_job());
    }

    #[test]
    fn test_cannot_accept_job_error() {
        let mut p = make_printer();
        p.set_error().unwrap();
        assert!(!p.can_accept_job());
    }

    #[test]
    fn test_drain_events_clears_buffer() {
        let mut p = make_printer();
        p.connect().unwrap();
        let first = p.drain_events();
        assert_eq!(first.len(), 1);
        let second = p.drain_events();
        assert!(second.is_empty());
    }

    #[test]
    fn test_connect_idempotent_no_duplicate_event() {
        let mut p = make_printer();
        p.connect().unwrap();
        let _ = p.drain_events();
        p.connect().unwrap(); // already Online — no-op
        assert!(p.drain_events().is_empty());
    }

    #[test]
    fn test_disconnect_idempotent_no_duplicate_event() {
        let mut p = make_printer(); // starts Offline
        p.disconnect().unwrap(); // already Offline — no-op
        assert!(p.drain_events().is_empty());
    }

    #[test]
    fn test_set_error_idempotent_no_duplicate_event() {
        let mut p = make_printer();
        p.set_error().unwrap();
        let _ = p.drain_events();
        p.set_error().unwrap(); // already Error — no-op
        assert!(p.drain_events().is_empty());
    }

    #[test]
    fn test_clone_has_empty_events() {
        let mut p = make_printer();
        p.connect().unwrap();
        let cloned = p.clone();
        // original still has event, clone does not
        assert_eq!(p.drain_events().len(), 1);
        let mut cloned = cloned;
        assert!(cloned.drain_events().is_empty());
    }
}
