// Printer Domain — Aggregate, Value Objects, Domain Events, Repository Trait

pub mod aggregate;
pub mod errors;
pub mod events;
pub mod repository;
pub mod value_objects;

pub use aggregate::Printer;
pub use errors::PrinterDomainError;
pub use events::{PrinterConnected, PrinterDisconnected, PrinterEvent};
pub use repository::PrinterRepository;
pub use value_objects::{PrinterId, PrinterName, PrinterStatus, PrinterType};
