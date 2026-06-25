// Entry point for Tauri application
// This file initializes the Tauri runtime and registers commands

// Prevents additional console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use sapo_printer::infrastructure::database::{
    run_migrations, DbPool, SqliteEventStore, SqlitePrintJobRepository,
};
use sapo_printer::infrastructure::downloader::ReqwestDownloader;
use sapo_printer::infrastructure::eventbus::tauri_event_bus::TauriEventBus;
use sapo_printer::infrastructure::metrics::MetricsCollector;
use sapo_printer::infrastructure::printer::PrinterManager;
use sapo_printer::infrastructure::queue::{QueueWorker, SqliteQueueManager};
use sapo_printer::infrastructure::secrets::SecretManager;
use sapo_printer::interface::tauri::dtos::printer_dto::{
    PrinterConfigDto, PrinterDto, PrinterStatusDto,
};
use sapo_printer::shared::event_bus::EventBus;
use sapo_printer::shared::logger::init_logging;
use sapo_printer::AppContextState;
use std::sync::Arc;
use tauri::Manager;

/// Tauri command: create print job(s) via CreatePrintJobUseCase.
#[tauri::command]
fn create_print_job(
    payload: sapo_printer::interface::tauri::commands::print_job::CreateJobPayload,
    ctx: tauri::State<'_, AppContextState>,
) -> Result<Vec<String>, String> {
    sapo_printer::interface::tauri::commands::print_job::execute_create_print_job(
        payload,
        ctx.inner(),
    )
}

/// Tauri command: cancel a print job via CancelPrintJobUseCase.
#[tauri::command]
fn cancel_print_job(
    payload: sapo_printer::interface::tauri::commands::print_job::CancelJobPayload,
    ctx: tauri::State<'_, AppContextState>,
) -> Result<(), String> {
    sapo_printer::interface::tauri::commands::print_job::execute_cancel_print_job(
        payload,
        ctx.inner(),
    )
}

/// Tauri command: list print jobs with filtering.
#[tauri::command]
fn list_jobs(
    filter: sapo_printer::application::dto::JobFilterDto,
    ctx: tauri::State<'_, AppContextState>,
) -> Result<Vec<sapo_printer::application::dto::JobDto>, String> {
    sapo_printer::interface::tauri::commands::print_job::execute_list_jobs(filter, ctx.inner())
}

/// Tauri command: get job status by ID.
#[tauri::command]
fn get_job_status(
    job_id: String,
    ctx: tauri::State<'_, AppContextState>,
) -> Result<sapo_printer::application::dto::JobDto, String> {
    sapo_printer::interface::tauri::commands::print_job::execute_get_job_status(job_id, ctx.inner())
}

/// Tauri command: get audit trail for a job.
#[tauri::command]
fn get_job_audit_trail(
    job_id: String,
    ctx: tauri::State<'_, AppContextState>,
) -> Result<sapo_printer::interface::tauri::dtos::audit_trail::AuditTrailResponse, String> {
    sapo_printer::interface::tauri::commands::audit_trail::execute_get_job_audit_trail(
        job_id,
        ctx.inner(),
    )
}

/// Tauri command: get operational metrics.
#[tauri::command]
fn get_metrics(
    ctx: tauri::State<'_, AppContextState>,
) -> Result<sapo_printer::interface::tauri::dtos::metrics::MetricsDto, String> {
    sapo_printer::interface::tauri::commands::metrics::execute_get_metrics(ctx.inner())
}

/// Tauri command: check for available updates.
#[tauri::command]
async fn check_for_updates(
    app: tauri::AppHandle,
) -> Result<sapo_printer::interface::tauri::dtos::update::UpdateCheckResponse, String> {
    sapo_printer::interface::tauri::commands::update::execute_check_for_updates(&app).await
}

/// Tauri command: download and install update.
#[tauri::command]
async fn install_update(
    app: tauri::AppHandle,
    ctx: tauri::State<'_, AppContextState>,
) -> Result<(), String> {
    sapo_printer::interface::tauri::commands::update::execute_install_update(
        &app,
        &ctx.install_guard,
        &ctx.last_emitted_update_version,
    )
    .await
}

/// Tauri command: restart the application after update.
#[tauri::command]
fn restart_app() -> Result<(), String> {
    sapo_printer::interface::tauri::commands::update::execute_restart_app()
}

#[cfg(target_os = "windows")]
use sapo_printer::infrastructure::printer::windows::Win32PrinterManager;
#[cfg(target_os = "windows")]
use sapo_printer::infrastructure::secrets::WindowsCredentialManager;

#[cfg(not(target_os = "windows"))]
use sapo_printer::infrastructure::printer::cups::CupsPrinterManager;

#[cfg(target_os = "macos")]
use sapo_printer::infrastructure::secrets::MacOSKeychain;

#[cfg(target_os = "linux")]
use sapo_printer::infrastructure::secrets::LinuxSecretService;

/// List all available printers (discovered from OS)
#[tauri::command]
fn list_printers(app_ctx: tauri::State<AppContextState>) -> Result<Vec<PrinterDto>, String> {
    let discovered = app_ctx.printer_manager.discover_printers();

    let dtos: Vec<PrinterDto> = discovered
        .iter()
        .map(|printer| {
            let printer_name = printer.name().as_str();

            PrinterDto {
                name: printer_name.to_string(),
                device_id: printer_name.to_string(),
                status: match printer.status() {
                    sapo_printer::domain::printer::PrinterStatus::Online => "Online".to_string(),
                    sapo_printer::domain::printer::PrinterStatus::Offline => "Offline".to_string(),
                    sapo_printer::domain::printer::PrinterStatus::Error => "Error".to_string(),
                },
                printer_type: match printer.printer_type() {
                    sapo_printer::domain::printer::PrinterType::Local => "Local".to_string(),
                    sapo_printer::domain::printer::PrinterType::Network => "Network".to_string(),
                },
                is_default: None,
            }
        })
        .collect();

    Ok(dtos)
}

/// Save printer configuration
#[tauri::command]
fn save_printer_config(
    config: PrinterConfigDto,
    _app_ctx: tauri::State<AppContextState>,
) -> Result<(), String> {
    use sapo_printer::infrastructure::config_store;

    // 1. Validate config fields
    // Paper size validation
    if config.paper_size.is_empty() {
        return Err("Khổ giấy không được để trống".to_string());
    }

    // Dimensions validation (50-500mm range when provided)
    if let Some(width) = config.paper_width {
        if !(50..=500).contains(&width) {
            return Err("Chiều rộng giấy phải trong khoảng 50-500mm".to_string());
        }
    }

    if let Some(height) = config.paper_height {
        if !(50..=500).contains(&height) {
            return Err("Chiều cao giấy phải trong khoảng 50-500mm".to_string());
        }
    }

    // For Custom paper size, dimensions are required
    if config.paper_size == "Custom" {
        if config.paper_width.is_none() {
            return Err("Chiều rộng giấy bắt buộc khi chọn khổ Custom".to_string());
        }
        if config.paper_height.is_none() {
            return Err("Chiều cao giấy bắt buộc khi chọn khổ Custom".to_string());
        }
    }

    // Margins validation (0-100mm range)
    if config.margin_left > 100 {
        return Err("Lề trái phải trong khoảng 0-100mm".to_string());
    }
    if config.margin_right > 100 {
        return Err("Lề phải phải trong khoảng 0-100mm".to_string());
    }
    if config.margin_top > 100 {
        return Err("Lề trên phải trong khoảng 0-100mm".to_string());
    }
    if config.margin_bottom > 100 {
        return Err("Lề dưới phải trong khoảng 0-100mm".to_string());
    }

    // Printer name validation
    if config.printer_name.is_empty() {
        return Err("Tên máy in không được để trống".to_string());
    }

    // Buffer validation
    if config.enable_buffer {
        match config.buffer_size_kb {
            Some(size) if (1..=1024).contains(&size) => {
                // Valid buffer size
            }
            Some(size) => {
                return Err(format!(
                    "Kích thước buffer phải trong khoảng 1-1024 KB (nhận được: {} KB)",
                    size
                ));
            }
            None => {
                return Err("Kích thước buffer bắt buộc khi bật buffer".to_string());
            }
        }
    } else if config.buffer_size_kb.is_some() {
        return Err("Không thể đặt kích thước buffer khi buffer đã tắt".to_string());
    }

    // Color mode validation
    let valid_color_modes = ["RGB", "ARGB", "BGR", "GRAY", "BINARY"];
    if !valid_color_modes.contains(&config.color_mode.as_str()) {
        return Err(format!(
            "Loại ảnh in không hợp lệ: '{}'. Chỉ chấp nhận: RGB, ARGB, BGR, GRAY, BINARY",
            config.color_mode
        ));
    }

    // 2. Convert DTO to config store model
    let print_config = config_store::PrintConfig {
        printer_name: config.printer_name,
        paper_size: config.paper_size,
        paper_width: config.paper_width,
        paper_height: config.paper_height,
        orientation: config.orientation,
        margin_left: config.margin_left,
        margin_right: config.margin_right,
        margin_top: config.margin_top,
        margin_bottom: config.margin_bottom,
        color_mode: config.color_mode,
        print_as_image: config.print_as_image,
        enable_buffer: config.enable_buffer,
        buffer_size_kb: config.buffer_size_kb,
    };

    // 3. Save to JSON file
    config_store::save_config(&print_config)?;

    Ok(())
}

/// Get printer configuration (global app config)
#[tauri::command]
fn get_printer_config(
    _app_ctx: tauri::State<AppContextState>,
) -> Result<Option<PrinterConfigDto>, String> {
    use sapo_printer::infrastructure::config_store;

    let config = config_store::load_config()?;

    match config {
        Some(cfg) => Ok(Some(PrinterConfigDto {
            printer_name: cfg.printer_name,
            paper_size: cfg.paper_size,
            paper_width: cfg.paper_width,
            paper_height: cfg.paper_height,
            orientation: cfg.orientation,
            margin_left: cfg.margin_left,
            margin_right: cfg.margin_right,
            margin_top: cfg.margin_top,
            margin_bottom: cfg.margin_bottom,
            color_mode: cfg.color_mode,
            print_as_image: cfg.print_as_image,
            enable_buffer: cfg.enable_buffer,
            buffer_size_kb: cfg.buffer_size_kb,
        })),
        None => Ok(None),
    }
}

/// Get current printer status
#[tauri::command]
fn get_printer_status(
    name: String,
    app_ctx: tauri::State<AppContextState>,
) -> Result<PrinterStatusDto, String> {
    // 1. Access PrinterManager from AppContext
    // 2. Call printer_manager.get_status(name)
    let status = app_ctx.printer_manager.get_status(&name);

    // 3. Map PrinterStatus enum to String
    let status_str = match status {
        sapo_printer::domain::printer::PrinterStatus::Online => "Online".to_string(),
        sapo_printer::domain::printer::PrinterStatus::Offline => "Offline".to_string(),
        sapo_printer::domain::printer::PrinterStatus::Error => "Error".to_string(),
    };

    // 4. Return PrinterStatusDto
    Ok(PrinterStatusDto { status: status_str })
}

/// Tauri command: register this app as a Chrome Native Messaging host.
/// Accepts a comma-separated list of allowed extension IDs.
#[tauri::command]
fn register_native_host(allowed_origins: Option<String>) -> Result<(), String> {
    let origins: Vec<String> = allowed_origins
        .map(|s| s.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    sapo_printer::interface::native_messaging::registry::register_native_host(origins)
}

/// Native Messaging mode: initialize deps without Tauri, run stdin/stdout loop.
fn run_native_messaging_mode() -> Result<(), String> {
    // Initialize logging early for native messaging mode
    init_logging();

    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let data_dir = std::path::PathBuf::from(&home).join(".sapo-printer");
    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("Cannot create data directory: {}", e))?;

    let db_path = data_dir.join("config.db");
    let db_path_str = db_path
        .to_str()
        .ok_or_else(|| "Database path contains non-UTF-8".to_string())?
        .to_string();

    let pool = DbPool::new(&db_path_str)
        .map_err(|e| format!("Database init failed: {}", e))?;

    {
        let mut conn = pool.get();
        run_migrations(&mut conn)
            .map_err(|e| format!("Migration failed: {}", e))?;
    }

    let job_repo = Arc::new(SqlitePrintJobRepository::new(pool.get_arc()));

    #[cfg(target_os = "windows")]
    let secret_manager: Arc<dyn SecretManager> =
        Arc::new(WindowsCredentialManager::new());

    #[cfg(target_os = "macos")]
    let secret_manager: Arc<dyn SecretManager> = Arc::new(MacOSKeychain::new());

    #[cfg(target_os = "linux")]
    let secret_manager: Arc<dyn SecretManager> = Arc::new(
        match LinuxSecretService::new() {
            Ok(service) => service,
            Err(e) => {
                return Err(format!("Secret Service unavailable: {}", e));
            }
        },
    );

    let event_store = Arc::new(SqliteEventStore::new(pool.get_arc(), secret_manager));
    let event_bus: Arc<dyn EventBus> = Arc::new(sapo_printer::shared::event_bus::InMemoryEventBus::new());

    let queue_manager = Arc::new(SqliteQueueManager::new(pool.get_arc()));

    // Register PushToQueueHandler to listen for PrintJobCreated events
    let push_handler = Arc::new(sapo_printer::application::handlers::push_to_queue_handler::PushToQueueHandler::new(
        queue_manager.clone(),
    ));
    event_bus.subscribe("PrintJobCreated", push_handler);

    let metrics_collector = Arc::new(MetricsCollector::new(
        pool.get_arc(),
        queue_manager.clone(),
    ));

    #[cfg(target_os = "windows")]
    let printer_manager: Arc<dyn PrinterManager> = Arc::new(Win32PrinterManager::new());
    #[cfg(not(target_os = "windows"))]
    let printer_manager: Arc<dyn PrinterManager> = Arc::new(CupsPrinterManager::new());

    // Startup cleanup: purge events older than 30 days (best-effort)
    match sapo_printer::infrastructure::database::cleanup_old_events(&event_store, 30) {
        Ok(deleted) => {
            if deleted > 0 {
                tracing::info!(
                    target = "sapo_printer::startup",
                    deleted_events = deleted,
                    "Audit cleanup: deleted old events"
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                target = "sapo_printer::startup",
                error = %e,
                "Audit cleanup failed (non-fatal)"
            );
        }
    }

    sapo_printer::interface::native_messaging::run_native_messaging(
        job_repo,
        printer_manager,
        event_store,
        event_bus,
        metrics_collector,
    )
}

fn main() {
    // Initialize structured logging FIRST, before any other operations
    init_logging();

    let args: Vec<String> = std::env::args().collect();

    // Check for --register-native-host CLI flag (headless registration)
    // Supports: --register-native-host extId1,extId2
    if let Some(pos) = args.iter().position(|a| a == "--register-native-host") {
        let origins: Vec<String> = args
            .get(pos + 1)
            .map(|s| s.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
            .unwrap_or_default();
        match sapo_printer::interface::native_messaging::registry::register_native_host(origins) {
            Ok(()) => std::process::exit(0),
            Err(e) => {
                eprintln!("Failed to register native host: {}", e);
                std::process::exit(1);
            }
        }
    }

    // Check for --native-messaging flag (Chrome Native Messaging mode)
    if args.iter().any(|a| a == "--native-messaging") {
        let result = run_native_messaging_mode();
        if let Err(e) = &result {
            // Log to file, not stderr (would corrupt protocol)
            let home = std::env::var("USERPROFILE")
                .or_else(|_| std::env::var("HOME"))
                .unwrap_or_else(|_| ".".to_string());
            let log_path = std::path::PathBuf::from(&home)
                .join(".sapo-printer")
                .join("native-messaging.log");
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
                .and_then(|mut f| {
                    use std::io::Write;
                    writeln!(f, "[FATAL] {}", e)
                });
        }
        // Exit code 0 on success, 1 on fatal startup error.
        // Chrome uses the exit code to determine if the native host is healthy.
        match result {
            Ok(()) => std::process::exit(0),
            Err(_) => std::process::exit(1),
        }
    }

    // 1. Ensure ~/.sapo-printer/ data directory exists
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let data_dir = std::path::PathBuf::from(&home).join(".sapo-printer");
    std::fs::create_dir_all(&data_dir).unwrap_or_else(|e| {
        eprintln!("Cannot create data directory: {e}");
        std::process::exit(1);
    });
    // Create temp directory for PDF downloads
    let temp_dir = data_dir.join("temp");
    std::fs::create_dir_all(&temp_dir).unwrap_or_else(|e| {
        eprintln!("Cannot create temp directory: {e}");
        std::process::exit(1);
    });

    // Startup cleanup: remove orphaned .tmp files and old .pdf files (>24h)
    sapo_printer::infrastructure::temp_file::startup_cleanup(&temp_dir);

    let db_path = data_dir.join("config.db");
    let db_path_str = db_path.to_str().unwrap_or_else(|| {
        eprintln!("Database path contains non-UTF-8 characters");
        std::process::exit(1);
    })
    .to_string();

    // 2. Start Tauri — all dependency init moved into .setup() to access AppHandle
    tauri::Builder::default()
        .setup(move |app| {
            let app_handle = app.handle().clone();

            // Initialize database connection
            let pool = DbPool::new(&db_path_str).unwrap_or_else(|e| {
                eprintln!("Database init failed: {e}");
                std::process::exit(1);
            });

            // Run migrations
            {
                let mut conn = pool.get();
                run_migrations(&mut conn).unwrap_or_else(|e| {
                    eprintln!("Migration failed: {e}");
                    std::process::exit(1);
                });
            }

            // Initialize AppContext dependencies
            let job_repo = Arc::new(SqlitePrintJobRepository::new(pool.get_arc()));

            // Initialize secret manager
            #[cfg(target_os = "windows")]
            let secret_manager: Arc<dyn SecretManager> =
                Arc::new(WindowsCredentialManager::new());

            #[cfg(target_os = "macos")]
            let secret_manager: Arc<dyn SecretManager> = Arc::new(MacOSKeychain::new());

            #[cfg(target_os = "linux")]
            let secret_manager: Arc<dyn SecretManager> = Arc::new(
                match LinuxSecretService::new() {
                    Ok(service) => service,
                    Err(e) => {
                        eprintln!("Warning: Secret Service unavailable: {}", e);
                        eprintln!(
                            "Device tokens and secrets will not be persisted securely."
                        );
                        eprintln!(
                            "Install gnome-keyring or use environment variables for secrets."
                        );
                        std::process::exit(1);
                    }
                },
            );

            let event_store = Arc::new(SqliteEventStore::new(pool.get_arc(), secret_manager.clone()));
            let event_bus: Arc<dyn EventBus> =
                Arc::new(TauriEventBus::new(app_handle.clone()));
            let queue_manager: Arc<dyn sapo_printer::infrastructure::queue::QueueManager> =
                Arc::new(SqliteQueueManager::new(pool.get_arc()));

            // Register PushToQueueHandler to listen for PrintJobCreated events
            let push_handler = Arc::new(sapo_printer::application::handlers::push_to_queue_handler::PushToQueueHandler::new(
                queue_manager.clone(),
            ));
            event_bus.subscribe("PrintJobCreated", push_handler);
            tracing::info!(
                target = "sapo_printer::startup",
                "PushToQueueHandler registered for PrintJobCreated events"
            );

            #[cfg(target_os = "windows")]
            let printer_manager: Arc<dyn PrinterManager> = Arc::new(Win32PrinterManager::new());

            #[cfg(not(target_os = "windows"))]
            let printer_manager: Arc<dyn PrinterManager> = Arc::new(CupsPrinterManager::new());

            // Initialize QueueWorker dependencies
            let downloader = Arc::new(ReqwestDownloader::new());

            let renderer: Arc<
                dyn sapo_printer::infrastructure::renderer::DocumentRenderer,
            > = Arc::new(sapo_printer::infrastructure::renderer::PdfiumRenderer::new(300));

            #[cfg(target_os = "windows")]
            let printer_engine: Arc<
                dyn sapo_printer::infrastructure::printer::PrinterEngine,
            > = Arc::new(
                sapo_printer::infrastructure::printer::windows::WindowsPrinterEngine::new(),
            );

            #[cfg(not(target_os = "windows"))]
            let printer_engine: Arc<
                dyn sapo_printer::infrastructure::printer::PrinterEngine,
            > = Arc::new(
                sapo_printer::infrastructure::printer::cups::CupsPrinterEngine::new(),
            );

            // Create and start QueueWorker
            let worker = Arc::new(QueueWorker::new(
                Arc::clone(&queue_manager),
                job_repo.clone()
                    as Arc<dyn sapo_printer::domain::print_job::PrintJobRepository>,
                Arc::clone(&event_store),
                Arc::clone(&event_bus),
                downloader,
                renderer,
                printer_engine,
            ));

            worker.start().expect("Failed to start queue worker");
            println!("Queue worker started successfully");

            // Create MetricsCollector
            let metrics_collector = Arc::new(MetricsCollector::new(
                pool.get_arc(),
                queue_manager.clone(),
            ));

            // Startup cleanup: purge events older than 30 days (best-effort)
            match sapo_printer::infrastructure::database::cleanup_old_events(&event_store, 30) {
                Ok(deleted) => {
                    if deleted > 0 {
                        tracing::info!(
                            target = "sapo_printer::startup",
                            deleted_events = deleted,
                            "Audit cleanup: deleted old events"
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        target = "sapo_printer::startup",
                        error = %e,
                        "Audit cleanup failed (non-fatal)"
                    );
                }
            }

            // Register managed state
            app.manage(AppContextState {
                printer_manager,
                secret_manager,
                job_repo,
                event_store,
                event_bus,
                queue_manager,
                queue_worker: worker,
                metrics_collector,
                app_handle,
                install_guard: sapo_printer::infrastructure::updater::update_checker::InstallGuard::new(),
                last_emitted_update_version: std::sync::Mutex::new(None),
            });

            // Register updater plugin
            #[cfg(desktop)]
            app.handle().plugin(
                tauri_plugin_updater::Builder::new().build(),
            )?;

            // Spawn background update checker (startup + periodic every 24h)
            #[cfg(desktop)]
            {
                let update_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    use sapo_printer::interface::tauri::dtos::update::UpdateCheckResponse;
                    use tauri::Emitter;

                    // Startup check
                    match sapo_printer::infrastructure::updater::update_checker::check_for_updates(
                        &update_handle,
                    )
                    .await
                    {
                        Ok(result) if result.update_available => {
                            tracing::info!(
                                target = "sapo_printer::updater",
                                version = ?result.version,
                                "Update available on startup"
                            );
                            let state = update_handle.state::<sapo_printer::AppContextState>();
                            let mut last_emitted = state.last_emitted_update_version.lock().unwrap();
                            if result.version != *last_emitted {
                                let dto = UpdateCheckResponse {
                                    update_available: result.update_available,
                                    version: result.version.clone(),
                                    release_notes: result.release_notes,
                                };
                                let _ = update_handle.emit("update-available", &dto);
                                *last_emitted = result.version;
                            }
                        }
                        Ok(_) => {
                            tracing::info!(target = "sapo_printer::updater", "No update available");
                        }
                        Err(e) => {
                            tracing::warn!(
                                target = "sapo_printer::updater",
                                error = %e,
                                "Startup update check failed (non-fatal)"
                            );
                        }
                    }

                    // Periodic check every 24 hours
                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs(24 * 60 * 60)).await;
                        match sapo_printer::infrastructure::updater::update_checker::check_for_updates(
                            &update_handle,
                        )
                        .await
                        {
                            Ok(result) if result.update_available => {
                                tracing::info!(
                                    target = "sapo_printer::updater",
                                    version = ?result.version,
                                    "Update available (periodic check)"
                                );
                                let state = update_handle.state::<sapo_printer::AppContextState>();
                                let mut last_emitted = state.last_emitted_update_version.lock().unwrap();
                                if result.version != *last_emitted {
                                    let dto = UpdateCheckResponse {
                                        update_available: result.update_available,
                                        version: result.version.clone(),
                                        release_notes: result.release_notes,
                                    };
                                    let _ = update_handle.emit("update-available", &dto);
                                    *last_emitted = result.version;
                                }
                            }
                            Ok(_) => {
                                tracing::info!(
                                    target = "sapo_printer::updater",
                                    "No update (periodic check)"
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    target = "sapo_printer::updater",
                                    error = %e,
                                    "Periodic update check failed (non-fatal)"
                                );
                            }
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_printers,
            save_printer_config,
            get_printer_config,
            get_printer_status,
            create_print_job,
            cancel_print_job,
            list_jobs,
            get_job_status,
            get_job_audit_trail,
            get_metrics,
            register_native_host,
            check_for_updates,
            install_update,
            restart_app,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let state = window.state::<AppContextState>();
                if let Err(e) = state.queue_worker.stop() {
                    eprintln!("Warning: Failed to stop queue worker gracefully: {}", e);
                } else {
                    println!("Queue worker stopped gracefully");
                }
            }
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("Failed to start Tauri application:");
            eprintln!("  {e}");
            std::process::exit(1);
        });
}
