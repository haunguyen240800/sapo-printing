//! Application Ports — trait abstractions for infrastructure dependencies.
//!
//! Ports define the contracts that use cases depend on.
//! Concrete implementations live in the infrastructure layer and are injected
//! at startup via Dependency Injection.

pub mod config_provider;
pub mod document_download_service;
pub mod event_store;
pub mod metrics_provider;
pub mod print_service;
pub mod printer_manager;
pub mod queue_manager;
pub mod secret_manager;
pub mod temp_file_manager;

pub use config_provider::{ConfigProvider, PrintConfigSnapshot};
pub use document_download_service::DocumentDownloadService;
pub use event_store::{EventStore, StoredEventData};
pub use metrics_provider::{
    JobMetrics, MetricsProvider, MetricsSnapshot, PerformanceMetrics, PrinterJobStats,
    PrinterMetrics, QueueMetrics,
};
pub use print_service::PrintService;
pub use printer_manager::{PrinterAvailability, PrinterManager};
pub use queue_manager::{QueueError, QueueManager};
pub use secret_manager::SecretManager;
pub use temp_file_manager::{TempFileHandle, TempFileManager};
