// Entry point for Tauri application
// This file initializes the Tauri runtime and registers commands

// Prevents additional console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use sapo_printer::infrastructure::configs::db::{run_migrations, DbPool};
use sapo_printer::infrastructure::persistence::{SqliteEventStore, SqlitePrintJobRepository};
use sapo_printer::infrastructure::integrations::network::ReqwestDownloader;
use sapo_printer::infrastructure::integrations::pdf_engine::bitmap_strategy::BitmapRenderStrategy;
use sapo_printer::infrastructure::integrations::pdf_engine::pdfium_loader;
use sapo_printer::infrastructure::telemetry::metrics::MetricsCollector;
use sapo_printer::application::ports::{
    ConfigProvider, EventStore, MetricsProvider, PrinterManager, QueueManager, SecretManager,
    TempFileManager,
};
use sapo_printer::infrastructure::configs::app::JsonFileConfigProvider;
use sapo_printer::infrastructure::persistence::SqliteQueueManager;
use sapo_printer::infrastructure::platform::printer_api::SystemPrinterManager;
use sapo_printer::infrastructure::platform::printing::DefaultPrintService;
use sapo_printer::infrastructure::temp_file::{self, FilesystemTempFileManager};
use sapo_printer::infrastructure::worker::QueueWorker;
use sapo_printer::interface::tauri::job_status_emitter::JobStatusEmitter;
use sapo_printer::interface::tauri::dtos::printer_dto::{
    PrinterConfigDto, PrinterDto, PrinterStatusDto,
};
use sapo_printer::application::dto::create_print_job_request::CreatePrintJobRequest;
use sapo_printer::application::use_cases::create_print_job::CreatePrintJobUseCase;
use sapo_printer::application::errors::ApplicationError;
use sapo_printer::shared::event_bus::EventBus;
use sapo_printer::shared::logger::init_logging;
use sapo_printer::AppContextState;
use std::sync::Arc;
use tauri::Manager;

/// Tauri command: create print job(s) via CreatePrintJobUseCase.
#[tauri::command]
async fn create_print_job(
    payload: sapo_printer::interface::tauri::commands::print_job::CreateJobPayload,
    ctx: tauri::State<'_, AppContextState>,
) -> Result<Vec<String>, String> {
    tracing::info!(
        target = "sapo_printer::tauri_command",
        command = "create_print_job",
        "Command received, spawning blocking task"
    );

    // Clone dependencies to move into blocking task
    let job_repo = ctx.job_repo.clone();
    let event_store = ctx.event_store.clone();
    let event_bus = ctx.event_bus.clone();
    let config_provider = ctx.config_provider.clone();
    let printer_manager = ctx.printer_manager.clone();

    // Run in blocking task to avoid blocking async runtime
    let result = tokio::task::spawn_blocking(move || {
        // Catch panics to prevent silent crashes
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tracing::info!(
                target = "sapo_printer::tauri_command",
                "Inside blocking task, creating use case"
            );

            if payload.pdf_urls.is_empty() {
                return Err("Danh sách URLs không được rỗng".to_string());
            }
            if payload.pdf_urls.len() > 5000 {
                return Err(format!(
                    "Số lượng URLs vượt quá giới hạn 5000 (nhận được: {})",
                    payload.pdf_urls.len()
                ));
            }

            let use_case = CreatePrintJobUseCase {
                job_repo,
                event_store,
                event_bus,
                config_provider,
                printer_manager,
            };

            tracing::info!(
                target = "sapo_printer::tauri_command",
                "Executing use case"
            );

            let mut job_ids: Vec<String> = Vec::with_capacity(payload.pdf_urls.len());
            for url in payload.pdf_urls {
                let request = CreatePrintJobRequest { pdf_url: url };
                let id = use_case.execute(request).map_err(|e| match &e {
                    ApplicationError::PrinterNotAvailable { name } => {
                        format!("Máy in '{}' không khả dụng hoặc đang offline", name)
                    }
                    _ => format!("{}", e),
                })?;
                job_ids.push(id.to_string());
            }
            let result: Result<Vec<String>, String> = Ok(job_ids);

            tracing::info!(
                target = "sapo_printer::tauri_command",
                success = result.is_ok(),
                error = ?result.as_ref().err(),
                "Use case execution completed"
            );

            result
        }))
        .map_err(|panic| {
            let panic_msg = if let Some(s) = panic.downcast_ref::<&str>() {
                format!("Panic: {}", s)
            } else if let Some(s) = panic.downcast_ref::<String>() {
                format!("Panic: {}", s)
            } else {
                "Panic: Unknown panic occurred".to_string()
            };
            tracing::error!(
                target = "sapo_printer::tauri_command",
                error = %panic_msg,
                "Caught panic in use case execution"
            );
            panic_msg
        })?
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?;

    tracing::info!(
        target = "sapo_printer::tauri_command",
        "Command completed successfully"
    );

    result
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
    filter: sapo_printer::application::dto::PrintJobFilterDto,
    ctx: tauri::State<'_, AppContextState>,
) -> Result<Vec<sapo_printer::application::dto::PrintJobDto>, String> {
    sapo_printer::interface::tauri::commands::print_job::execute_list_jobs(filter, ctx.inner())
}

/// Tauri command: get job status by ID.
#[tauri::command]
fn get_job_status(
    job_id: String,
    ctx: tauri::State<'_, AppContextState>,
) -> Result<sapo_printer::application::dto::PrintJobDto, String> {
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
async fn get_metrics(
    ctx: tauri::State<'_, AppContextState>,
) -> Result<sapo_printer::interface::tauri::dtos::metrics::MetricsDto, String> {
    // Clone only what we need to avoid blocking
    let metrics_provider = ctx.metrics_provider.clone();

    // Run in blocking task to avoid blocking async runtime
    tokio::task::spawn_blocking(move || {
        use sapo_printer::application::use_cases::GetMetricsUseCase;
        let use_case = GetMetricsUseCase::new(metrics_provider);
        let snapshot = use_case.execute().map_err(|e| format!("{}", e))?;

        use sapo_printer::interface::tauri::dtos::metrics::*;
        Ok(MetricsDto {
            collected_at: snapshot.collected_at,
            job_metrics: JobMetricsDto {
                total_jobs: snapshot.job_metrics.total_jobs,
                pending: snapshot.job_metrics.pending,
                queued: snapshot.job_metrics.queued,
                downloaded: snapshot.job_metrics.downloaded,
                submitted: snapshot.job_metrics.submitted,
                printing: snapshot.job_metrics.printing,
                completed: snapshot.job_metrics.completed,
                failed: snapshot.job_metrics.failed,
                cancelled: snapshot.job_metrics.cancelled,
                success_rate: snapshot.job_metrics.success_rate,
            },
            queue_metrics: QueueMetricsDto {
                current_depth: snapshot.queue_metrics.current_depth,
                avg_wait_time_secs: snapshot.queue_metrics.avg_wait_time_secs,
            },
            printer_metrics: PrinterMetricsDto {
                printers: snapshot
                    .printer_metrics
                    .printers
                    .iter()
                    .map(|p| PrinterUsageDto {
                        printer_name: p.printer_name.clone(),
                        total_jobs: p.total_jobs,
                        completed_jobs: p.completed_jobs,
                        utilization_percent: p.utilization_percent,
                    })
                    .collect(),
            },
            performance_metrics: PerformanceMetricsDto {
                avg_job_duration_secs: snapshot.performance_metrics.avg_job_duration_secs,
                p50_job_duration_secs: snapshot.performance_metrics.p50_job_duration_secs,
                p95_job_duration_secs: snapshot.performance_metrics.p95_job_duration_secs,
                p99_job_duration_secs: snapshot.performance_metrics.p99_job_duration_secs,
                avg_download_time_secs: snapshot.performance_metrics.avg_download_time_secs,
                avg_render_time_secs: snapshot.performance_metrics.avg_render_time_secs,
                avg_print_time_secs: snapshot.performance_metrics.avg_print_time_secs,
            },
        })
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
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
use sapo_printer::infrastructure::platform::keychain::WindowsCredentialManager;


#[cfg(target_os = "macos")]
use sapo_printer::infrastructure::platform::keychain::MacOSKeychain;

#[cfg(target_os = "linux")]
use sapo_printer::infrastructure::platform::keychain::LinuxSecretService;

#[tauri::command]
fn list_printers(ctx: tauri::State<'_, AppContextState>) -> Result<Vec<PrinterDto>, String> {
    use sapo_printer::application::use_cases::ListPrintersUseCase;

    let use_case = ListPrintersUseCase::new(ctx.printer_manager.clone());

    match use_case.execute() {
        Ok(printers) => Ok(printers
            .into_iter()
            .map(|p| PrinterDto {
                name: p.name,
                device_id: p.id,
                status: p.status,
                printer_type: p.printer_type,
                is_default: Some(p.is_default),
            })
            .collect()),
        Err(e) => Err(format!("{:?}", e)),
    }
}

/// Save printer configuration
#[tauri::command]
fn save_printer_config(
    config: PrinterConfigDto,
    _app_ctx: tauri::State<AppContextState>,
) -> Result<(), String> {
    use sapo_printer::infrastructure::configs::app::app_print_config;

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
    let print_config = app_print_config::AppPrintConfig {
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
    app_print_config::save_config(&print_config)?;

    Ok(())
}

/// Get printer configuration (global app config)
#[tauri::command]
fn get_printer_config(
    _app_ctx: tauri::State<AppContextState>,
) -> Result<PrinterConfigDto, String> {
    use sapo_printer::infrastructure::configs::app::app_print_config;

    let config = app_print_config::load_config()?;

    match config {
        Some(cfg) => Ok(PrinterConfigDto {
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
        }),
        None => Ok(PrinterConfigDto {
            printer_name: String::new(),
            paper_size: String::new(),
            paper_width: None,
            paper_height: None,
            orientation: String::new(),
            margin_left: 0,
            margin_right: 0,
            margin_top: 0,
            margin_bottom: 0,
            color_mode: String::new(),
            print_as_image: false,
            enable_buffer: false,
            buffer_size_kb: None,
        }),
    }
}

/// Get current printer status
#[tauri::command]
fn get_printer_status(
    _name: String,
) -> Result<PrinterStatusDto, String> {
    Ok(PrinterStatusDto { status: "Online".to_string() })
}

#[derive(serde::Serialize)]
struct PrinterCategoryResult {
    category: String,
    needs_rendering: bool,
    needs_save_dialog: bool,
    description: String,
}

/// Detect printer category based on printer name
#[tauri::command]
fn detect_printer_category(printer_name: String) -> Result<PrinterCategoryResult, String> {
    let printer_lower = printer_name.to_lowercase();
    
    let pdf_patterns = [
        "microsoft print to pdf",
        "print to pdf",
        "save as pdf",
        "pdf printer",
        "adobe pdf",
        "foxit reader pdf printer",
        "nitro pdf creator",
        "cutepdf writer",
        "dopdf",
    ];
    
    let virtual_patterns = [
        "microsoft xps document writer",
        "microsoft print to image",
        "fax",
        "onenote",
        "send to onenote",
    ];
    
    let is_pdf = pdf_patterns.iter().any(|&p| printer_lower.contains(p));
    let is_virtual = virtual_patterns.iter().any(|&p| printer_lower.contains(p));
    
    if is_pdf {
        Ok(PrinterCategoryResult {
            category: "pdf".to_string(),
            needs_rendering: false,
            needs_save_dialog: true,
            description: "Print to PDF Virtual Printer".to_string(),
        })
    } else if is_virtual {
        Ok(PrinterCategoryResult {
            category: "virtual".to_string(),
            needs_rendering: false,
            needs_save_dialog: false,
            description: "Virtual/Image Printer".to_string(),
        })
    } else {
        Ok(PrinterCategoryResult {
            category: "physical".to_string(),
            needs_rendering: true,
            needs_save_dialog: false,
            description: "Physical Network/USB Printer".to_string(),
        })
    }
}



fn main() {
    // Initialize structured logging FIRST, before any other operations
    init_logging();




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

            // Register the Tauri resource directory so PDFium can locate its
            // bundled native library regardless of the current working dir.
            if let Ok(resource_dir) = app.path().resource_dir() {
                pdfium_loader::set_pdfium_resource_dir(resource_dir);
            }

            // Initialize database connection
            let pool = DbPool::new(&db_path_str).unwrap_or_else(|e| {
                eprintln!("Database init failed: {e}");
                std::process::exit(1);
            });

            // Run migrations
            {
                let mut conn = pool.get().unwrap_or_else(|e| {
                    eprintln!("Database connection failed: {e}");
                    std::process::exit(1);
                });
                run_migrations(&mut *conn).unwrap_or_else(|e| {
                    eprintln!("Migration failed: {e}");
                    std::process::exit(1);
                });
            }

            // Startup cleanup: retention is sourced from app_settings (see
            // migration 2 — `temp_file_retention_hours`), falling back to the
            // hardcoded default on any read failure.
            let retention = temp_file::load_retention(&pool);
            temp_file::startup_cleanup(&temp_dir, retention);

            // Initialize AppContext dependencies
            let job_repo = Arc::new(SqlitePrintJobRepository::new(pool.clone()));

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

            let event_store = Arc::new(SqliteEventStore::new(pool.clone(), secret_manager.clone()));

            // EventBus is just a pub/sub dispatcher (handlers run synchronously
            // in the publisher thread). UI emission is a SEPARATE subscriber:
            // `JobStatusEmitter` lives in the interface layer.
            let event_bus: Arc<dyn EventBus> =
                Arc::new(sapo_printer::shared::event_bus::InMemoryEventBus::new());

            // Forward `PrintJob*` events to the Tauri frontend.
            JobStatusEmitter::new(app_handle.clone()).register(&event_bus);

            let queue_manager: Arc<dyn QueueManager> =
                Arc::new(SqliteQueueManager::new(pool.clone()));

            // Register PushToQueueHandler to listen for PrintJobCreated events
            let push_handler = Arc::new(sapo_printer::application::handlers::push_to_queue_handler::PushToQueueHandler::new(
                queue_manager.clone(),
            ));
            event_bus.subscribe("PrintJobCreated", push_handler);
            tracing::info!(
                target = "sapo_printer::startup",
                "PushToQueueHandler registered for PrintJobCreated events"
            );


            // Initialize QueueWorker dependencies
            let downloader: Arc<dyn sapo_printer::application::ports::DocumentDownloadService> =
                Arc::new(ReqwestDownloader::new());
            let render_strategy: Arc<dyn sapo_printer::infrastructure::integrations::pdf_engine::renderer::RenderStrategy> =
                Arc::new(BitmapRenderStrategy::new().map_err(|e| e.to_string())?);
            let print_service: Arc<dyn sapo_printer::application::ports::PrintService> =
                Arc::new(DefaultPrintService::new(render_strategy));

            // Filesystem temp file manager (owns the per-app temp directory)
            let temp_files: Arc<dyn TempFileManager> =
                Arc::new(FilesystemTempFileManager::new(temp_dir.clone()));

            // OS-backed PrinterManager — used both for ONLINE availability checks
            // and for the `list_printers` Tauri command.
            let printer_manager: Arc<dyn PrinterManager> =
                Arc::new(SystemPrinterManager::new());

            // Config provider — JSON file backed
            let config_provider: Arc<dyn ConfigProvider> =
                Arc::new(JsonFileConfigProvider::new());

            // Cast event_store to the application port trait for DI
            let event_store_port: Arc<dyn EventStore> = Arc::clone(&event_store) as Arc<dyn EventStore>;

            let job_repo_port: Arc<dyn sapo_printer::domain::print_job::PrintJobRepository> =
                job_repo.clone();

            let process_use_case = Arc::new(
                sapo_printer::application::use_cases::ProcessPrintJobUseCase::new(
                    Arc::clone(&job_repo_port),
                    Arc::clone(&event_store_port),
                    Arc::clone(&event_bus),
                    Arc::clone(&downloader),
                    Arc::clone(&print_service),
                    Arc::clone(&temp_files),
                    Arc::clone(&config_provider),
                ),
            );

            let failure_handler = Arc::new(
                sapo_printer::application::handlers::job_failure_handler::JobFailureHandler::new(
                    Arc::clone(&job_repo_port),
                    Arc::clone(&event_store_port),
                    Arc::clone(&event_bus),
                ),
            );

            // Create and start QueueWorker
            let worker = Arc::new(QueueWorker::new(
                Arc::clone(&queue_manager),
                Arc::clone(&job_repo_port),
                process_use_case,
                failure_handler,
            ));

            worker.start().expect("Failed to start queue worker");
            println!("Queue worker started successfully");

            // Create MetricsCollector (concrete) and cast to MetricsProvider port
            let metrics_provider: Arc<dyn MetricsProvider> = Arc::new(MetricsCollector::new(
                pool.clone(),
                queue_manager.clone(),
            ));

            // Startup cleanup: purge events older than 30 days (best-effort)
            match sapo_printer::application::services::audit_service::cleanup_old_events(&event_store_port, 30) {
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
                secret_manager,
                job_repo,
                event_store: Arc::clone(&event_store) as Arc<dyn EventStore>,
                event_bus,
                queue_manager,
                queue_worker: worker,
                metrics_provider,
                config_provider,
                printer_manager,
                temp_files,
                app_handle,
                install_guard: sapo_printer::infrastructure::platform::updater::update_checker::InstallGuard::new(),
                last_emitted_update_version: std::sync::Mutex::new(None),
            });

            // ==================== HTTPS local server bootstrap ====================
            // Requires helper service `sapo-printer-agent` đã sinh cert vào data_dir/tls/.
            {
                use sapo_printer::interface::http_server;
                use sapo_printer::interface::tauri::commands::agent::AgentState;

                let bootstrap_data_dir = data_dir.clone();
                // Cert dùng chung với cert-manager (ProgramData\SapoPrinter trên Windows).
                // App đọc cert đã được installer provision + trust, không tự cài CA.
                let bootstrap_cert_dir =
                    sapo_printer::infrastructure::platform::tls::shared_cert_dir();
                let bootstrap_pool = pool.clone();
                let bootstrap_ctx = app.state::<AppContextState>();
                let bootstrap_event_bus = bootstrap_ctx.event_bus.clone();
                let bootstrap_job_repo = bootstrap_ctx.job_repo.clone()
                    as Arc<dyn sapo_printer::domain::print_job::PrintJobRepository>;
                let bootstrap_event_store = bootstrap_ctx.event_store.clone();
                let bootstrap_config_provider = bootstrap_ctx.config_provider.clone();
                let bootstrap_printer_manager = bootstrap_ctx.printer_manager.clone();
                let bootstrap_use_cases = Arc::new(
                    sapo_printer::application::services::UseCaseFactory::new(
                        bootstrap_job_repo,
                        bootstrap_event_store,
                        bootstrap_event_bus.clone(),
                        bootstrap_config_provider,
                        bootstrap_printer_manager,
                    ),
                );
                let app_handle_for_agent = app.handle().clone();

                tauri::async_runtime::spawn(async move {
                    match http_server::start_bootstrap(
                        &bootstrap_cert_dir,
                        &bootstrap_data_dir,
                        bootstrap_pool,
                        bootstrap_event_bus,
                        bootstrap_use_cases,
                        env!("CARGO_PKG_VERSION"),
                        "1.0.0",
                    )
                    .await
                    {
                        Ok(result) => {
                            tracing::info!(port = result.port, "HTTPS agent started");
                            // Wire pair request → Tauri emit → front-end toast.
                            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                            result.token_manager.set_ui_sink(tx).await;
                            let emit_handle = app_handle_for_agent.clone();
                            tauri::async_runtime::spawn(async move {
                                use tauri::Emitter;
                                while let Some(req) = rx.recv().await {
                                    let payload = serde_json::json!({
                                        "request_id": req.request_id.to_string(),
                                        "origin": req.origin,
                                    });
                                    let _ = emit_handle.emit("agent-pair-request", payload);
                                }
                            });
                            app_handle_for_agent.manage(AgentState {
                                token_manager: result.token_manager,
                                agent_port: result.port,
                            });
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "HTTPS agent bootstrap failed — webapp integration disabled");
                        }
                    }
                });
            }

            // Register dialog plugin for native file dialogs
            app.handle().plugin(tauri_plugin_dialog::init())?;

            // ==================== AUTO-UPDATE ====================
            // Cơ chế check version / auto-update qua tauri-plugin-updater.
            // NOTE(local-publish): việc phát hành `latest.json` ở LOCAL đã bị gỡ
            // (`scripts/update-latest-json.mjs` + hook trong `pnpm build` + file
            // `installers/latest.json`). Sẽ thay bằng CI phát hành theo git tag `v*`
            // (GitLab CI/CD) tự sinh + upload `latest.json` như release artifact.
            // Endpoint trong `tauri.conf.json` cần trỏ về GitLab release sau này.

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
                    match sapo_printer::infrastructure::platform::updater::update_checker::check_for_updates(
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
                        match sapo_printer::infrastructure::platform::updater::update_checker::check_for_updates(
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
            detect_printer_category,
            create_print_job,
            cancel_print_job,
            list_jobs,
            get_job_status,
            get_job_audit_trail,
            get_metrics,
            check_for_updates,
            install_update,
            restart_app,
            sapo_printer::interface::tauri::commands::agent::get_agent_port,
            sapo_printer::interface::tauri::commands::agent::get_paired_origins,
            sapo_printer::interface::tauri::commands::agent::revoke_token,
            sapo_printer::interface::tauri::commands::agent::resolve_pair,
            sapo_printer::interface::tauri::commands::agent::renew_cert_now,
            sapo_printer::interface::tauri::commands::agent::get_agent_status,
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
