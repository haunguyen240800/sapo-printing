// Printer Domain — Aggregate, Value Objects, Domain Events

pub mod aggregate;
pub mod errors;
pub mod events;
pub mod value_objects;

pub use aggregate::Printer;
pub use errors::PrinterDomainError;
pub use events::{PrinterConnected, PrinterDisconnected, PrinterEvent};
pub use value_objects::{PrinterId, PrinterName, PrinterStatus, PrinterType};
