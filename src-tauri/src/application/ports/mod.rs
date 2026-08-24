//! Application Ports — trait abstractions for infrastructure dependencies.
//!
//! Ports define the contracts that use cases depend on.
//! Concrete implementations live in the infrastructure layer and are injected
//! at startup via Dependency Injection.

pub mod api_token_port;
pub mod config_port;
pub mod download_port;
pub mod event_bus;
pub mod event_store;
pub mod metrics_port;
pub mod print_port;
pub mod printer_port;
pub mod queue_port;
pub mod secret_port;
pub mod temp_file_port;

pub use api_token_port::{
    ApiTokenPort, IssuedToken, PairError, PairRequestSink, PairedOrigin, PendingPairRequest,
};
pub use config_port::{ConfigPort, PrintConfigSnapshot};
pub use download_port::DownloadPort;
pub use event_bus::{EventBus, EventBusError, EventHandler};
pub use event_store::{EventStore, StoredEventData};
pub use metrics_port::{MetricsPort, MetricsSnapshot};
pub use print_port::PrintPort;
pub use printer_port::{PrinterAvailability, PrinterPort};
pub use queue_port::{QueueError, QueuePort};
pub use secret_port::SecretPort;
pub use temp_file_port::{TempFileHandle, TempFilePort};
