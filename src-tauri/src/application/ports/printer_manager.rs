//! `PrinterManager` port — abstracts OS-level printer discovery and status queries.
//!
//! The application treats printers as external (OS-owned) resources, not as
//! domain aggregates. This port is the single boundary the use cases cross to
//! ask the operating system about printer availability or to enumerate
//! installed printers.

use crate::application::dto::PrinterDto;
use crate::shared::errors::InfrastructureError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrinterAvailability {
    /// Printer exists and is accepting jobs.
    Online,
    /// Printer exists but cannot accept jobs right now (offline, paused, error).
    Offline,
    /// Printer name was not found on the host.
    Unknown,
}

pub trait PrinterManager: Send + Sync {
    /// Enumerate every printer registered with the OS spooler.
    fn list(&self) -> Result<Vec<PrinterDto>, InfrastructureError>;

    /// Return the availability of the named printer.
    fn availability(&self, printer_id: &str) -> Result<PrinterAvailability, InfrastructureError>;
}
