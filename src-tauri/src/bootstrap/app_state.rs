use std::sync::Arc;

use crate::{
    AppContextState,
    application::{
        handlers::{
            print_job_created_handler::PrintJobCreatedHandler,
            print_job_failed_handler::PrintJobFailedHandler,
        },
        ports::{
            event_bus::EventBus, ConfigPort, DownloadPort, EventStore, MetricsPort, PrintPort,
            PrinterPort, QueuePort, SecretPort, TempFilePort,
        },
        services::audit_service,
        use_cases::{
            CreatePrintJobUseCase, GetAuditTrailUseCase, GetJobStatusUseCase, GetMetricsUseCase,
            ListPrintersUseCase, ProcessPrintJobUseCase,
        },
    },
    infrastructure::{
        configs::app::JsonFileConfigProvider,
        configs::db::DbPool,
        eventbus::InMemoryEventBus,
        integrations::{
            network::ReqwestDownloader,
            pdf_engine::{bitmap_strategy::BitmapRenderStrategy, pdfium_loader},
        },
        persistence::{EventRepository, JobQueueBroker, PrintJobRepository},
        platform::{
            printer_api::SystemPrinterManager,
            printing::DefaultPrintService,
            updater::update_checker::InstallGuard,
        },
        telemetry::metrics::MetricsCollector,
        temp_file::FilesystemTempFileManager,
        worker::QueueWorker,
    },
};

#[cfg(target_os = "windows")]
use crate::infrastructure::platform::keychain::WindowsCredentialManager;
#[cfg(target_os = "macos")]
use crate::infrastructure::platform::keychain::MacOSKeychain;
#[cfg(target_os = "linux")]
use crate::infrastructure::platform::keychain::LinuxSecretService;

use tauri::AppHandle;

/// Wires up tất cả dependencies và trả về `AppContextState` sẵn sàng `.manage()`.
pub fn build_app_state(
    pool: DbPool,
    temp_dir: &std::path::Path,
    app_handle: AppHandle,
    resource_dir: Option<std::path::PathBuf>,
) -> AppContextState {
    // --- PDFium resource dir ---
    if let Some(dir) = resource_dir {
        pdfium_loader::set_pdfium_resource_dir(dir);
    }

    // --- Repositories ---
    let job_repo: Arc<dyn crate::domain::print_job::PrintJobRepository> =
        Arc::new(PrintJobRepository::new(pool.clone()));

    // --- Secret manager (platform-specific) ---
    #[cfg(target_os = "windows")]
    let secret_manager: Arc<dyn SecretPort> = Arc::new(WindowsCredentialManager::new());

    #[cfg(target_os = "macos")]
    let secret_manager: Arc<dyn SecretPort> = Arc::new(MacOSKeychain::new());

    #[cfg(target_os = "linux")]
    let secret_manager: Arc<dyn SecretPort> = Arc::new(
        match LinuxSecretService::new() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Warning: Secret Service unavailable: {e}");
                eprintln!("Device tokens and secrets will not be persisted securely.");
                eprintln!("Install gnome-keyring or use environment variables for secrets.");
                std::process::exit(1);
            }
        },
    );

    // --- Event store & bus ---
    let event_store: Arc<dyn EventStore> =
        Arc::new(EventRepository::new(pool.clone(), secret_manager.clone()));

    let event_bus: Arc<dyn EventBus> = Arc::new(InMemoryEventBus::new());

    crate::interface::tauri::print_job_event_emitter::PrintJobEventEmitter::new(app_handle.clone())
        .register(&event_bus);

    // --- Queue ---
    let queue_manager: Arc<dyn QueuePort> = Arc::new(JobQueueBroker::new(pool.clone()));

    event_bus.subscribe(
        "PrintJobCreated",
        Arc::new(PrintJobCreatedHandler::new(queue_manager.clone())),
    );
    tracing::info!(
        target = "sapo_printer::startup",
        "PrintJobCreatedHandler registered"
    );

    // --- Infrastructure ports ---
    let downloader: Arc<dyn DownloadPort> = Arc::new(ReqwestDownloader::new());
    let render_strategy = Arc::new(
        BitmapRenderStrategy::new()
            .unwrap_or_else(|e| {
                eprintln!("BitmapRenderStrategy init failed: {e}");
                std::process::exit(1);
            }),
    );
    let print_service: Arc<dyn PrintPort> =
        Arc::new(DefaultPrintService::new(render_strategy));
    let temp_files: Arc<dyn TempFilePort> =
        Arc::new(FilesystemTempFileManager::new(temp_dir.to_path_buf()));
    let printer_manager: Arc<dyn PrinterPort> = Arc::new(SystemPrinterManager::new());
    let config_provider: Arc<dyn ConfigPort> = Arc::new(JsonFileConfigProvider::new());

    // --- Use cases ---
    let process_use_case = Arc::new(ProcessPrintJobUseCase::new(
        Arc::clone(&job_repo),
        Arc::clone(&event_store),
        Arc::clone(&event_bus),
        Arc::clone(&downloader),
        Arc::clone(&print_service),
        Arc::clone(&temp_files),
    ));

    let failure_handler = Arc::new(PrintJobFailedHandler::new(
        Arc::clone(&job_repo),
        Arc::clone(&event_store),
        Arc::clone(&event_bus),
    ));

    let worker = Arc::new(QueueWorker::new(
        Arc::clone(&queue_manager),
        Arc::clone(&job_repo),
        process_use_case,
        failure_handler,
    ));
    worker.start().expect("Failed to start queue worker");
    tracing::info!(target = "sapo_printer::startup", "Queue worker started");

    // --- Metrics ---
    let metrics_provider: Arc<dyn MetricsPort> =
        Arc::new(MetricsCollector::new(pool.clone(), queue_manager.clone()));

    // --- Audit cleanup ---
    match audit_service::cleanup_old_events(&event_store, 30) {
        Ok(n) if n > 0 => tracing::info!(
            target = "sapo_printer::startup",
            deleted_events = n,
            "Audit cleanup done"
        ),
        Err(e) => tracing::warn!(
            target = "sapo_printer::startup",
            error = %e,
            "Audit cleanup failed (non-fatal)"
        ),
        _ => {}
    }

    AppContextState {
        create_print_job_uc: Arc::new(CreatePrintJobUseCase {
            job_repo: Arc::clone(&job_repo),
            event_store: Arc::clone(&event_store),
            event_bus: Arc::clone(&event_bus),
            config_provider: Arc::clone(&config_provider),
            printer_manager: Arc::clone(&printer_manager),
        }),
        get_job_status_uc: Arc::new(GetJobStatusUseCase::new(Arc::clone(&job_repo))),
        get_metrics_uc: Arc::new(GetMetricsUseCase::new(Arc::clone(&metrics_provider))),
        get_audit_trail_uc: Arc::new(GetAuditTrailUseCase::new(
            Arc::clone(&event_store),
            Arc::clone(&secret_manager),
        )),
        list_printers_uc: Arc::new(ListPrintersUseCase::new(Arc::clone(&printer_manager))),
        queue_worker: worker,
        app_handle,
        install_guard: InstallGuard::new(),
        last_emitted_update_version: std::sync::Mutex::new(None),
        pending_update: std::sync::Mutex::new(None),
    }
}
