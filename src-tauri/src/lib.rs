// Library root - declares 4-layer Clean Architecture modules
// This file is the entry point for the library crate

use std::sync::Arc;

// Interface Layer - External-facing APIs
pub mod interface;

// Application Layer - Use Cases and Application Services
pub mod application;

// Domain Layer - Core Business Logic (PURE - NO EXTERNAL DEPENDENCIES)
pub mod domain;

// Infrastructure Layer - External System Implementations
pub mod infrastructure;

// Shared Layer - Cross-Cutting Concerns
pub mod shared;

/// Shared state registered with Tauri via `.manage()`.
/// Commands access this via `tauri::State<'_, AppContextState>`.
pub struct AppContextState {
    pub printer_repo: Arc<dyn domain::printer::PrinterRepository>,
    pub printer_manager: Arc<dyn infrastructure::printer::PrinterManager>,
    pub _secret_manager: Arc<dyn infrastructure::secrets::SecretManager>,
    pub job_repo: Arc<dyn domain::print_job::PrintJobRepository>,
    pub event_store: Arc<infrastructure::database::SqliteEventStore>,
    pub event_bus: Arc<dyn shared::event_bus::EventBus>,
    pub queue_manager: Arc<dyn infrastructure::queue::QueueManager>,
    pub queue_worker: Arc<infrastructure::queue::QueueWorker>,
}
