//! Print Job bounded context.
//!
//! Aggregate root: `PrintJob` (owns the lifecycle state machine and emits domain
//! events; carries the document reference and rendered-file location directly).
//! Value objects: `PrintJobId`, `PrinterId`, `PrintStatus`, `PaperSize`,
//! `PrintJobSettings`, `Transform`.
//!
//! `PrinterId` lives here (not in a separate `domain/printer/` context) because
//! the OS-owned printer resource is not modelled as an aggregate in this
//! application — `PrintJob` references it by identity only.

pub mod aggregate;
pub mod errors;
pub mod events;
pub mod repository;
pub mod services;
pub mod value_objects;

pub use aggregate::{MAX_RETRY_COUNT, PrintJob};
pub use errors::PrintJobError;
pub use repository::PrintJobRepository;
pub use value_objects::{
    PaperSize, PrintJobId, PrintJobSettings, PrintStatus, PrinterId, Transform,
};
