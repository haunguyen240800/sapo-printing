// Library root - declares 4-layer Clean Architecture modules
// This file is the entry point for the library crate

use std::sync::Arc;

use application::use_cases::{
    CancelPrintJobUseCase, CreatePrintJobUseCase, GetAuditTrailUseCase, GetJobStatusUseCase,
    GetMetricsUseCase, ListPrintJobsUseCase, ListPrintersUseCase,
};

// Interface Layer - External-facing APIs
pub mod interface;

// Application Layer - Use Cases and Application Services
pub mod application;

// Domain Layer - Core Business Logic (PURE - NO EXTERNAL DEPENDENCIES)
pub mod domain;

// Infrastructure Layer - External System Implementations
pub mod infrastructure;

/// Shared state registered with Tauri via `.manage()`.
/// Commands access this via `tauri::State<'_, AppContextState>`.
///
/// Holds pre-built application use cases (assembled in the composition root).
/// The interface layer invokes these use cases and never touches domain
/// repositories or infrastructure ports directly.
pub struct AppContextState {
    pub create_print_job_uc: Arc<CreatePrintJobUseCase>,
    pub cancel_print_job_uc: Arc<CancelPrintJobUseCase>,
    pub list_print_jobs_uc: Arc<ListPrintJobsUseCase>,
    pub get_job_status_uc: Arc<GetJobStatusUseCase>,
    pub get_metrics_uc: Arc<GetMetricsUseCase>,
    pub get_audit_trail_uc: Arc<GetAuditTrailUseCase>,
    pub list_printers_uc: Arc<ListPrintersUseCase>,
    pub queue_worker: Arc<infrastructure::worker::QueueWorker>,
    pub app_handle: tauri::AppHandle,
    pub install_guard: infrastructure::platform::updater::update_checker::InstallGuard,
    pub last_emitted_update_version: std::sync::Mutex<Option<String>>,
}
