// Library root - declares 4-layer Clean Architecture modules
// This file is the entry point for the library crate

use std::sync::Arc;

use application::ports::{
    ConfigProvider, EventStore, MetricsProvider, PrinterManager, QueueManager, SecretManager,
    TempFileManager,
};

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
    pub secret_manager: Arc<dyn SecretManager>,
    pub job_repo: Arc<dyn domain::print_job::PrintJobRepository>,
    pub event_store: Arc<dyn EventStore>,
    pub event_bus: Arc<dyn shared::event_bus::EventBus>,
    pub queue_manager: Arc<dyn QueueManager>,
    pub queue_worker: Arc<infrastructure::worker::QueueWorker>,
    pub metrics_provider: Arc<dyn MetricsProvider>,
    pub config_provider: Arc<dyn ConfigProvider>,
    pub printer_manager: Arc<dyn PrinterManager>,
    pub temp_files: Arc<dyn TempFileManager>,
    pub app_handle: tauri::AppHandle,
    pub install_guard: infrastructure::platform::updater::update_checker::InstallGuard,
    pub last_emitted_update_version: std::sync::Mutex<Option<String>>,
}
