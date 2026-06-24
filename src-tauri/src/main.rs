// Entry point for Tauri application
// This file initializes the Tauri runtime and registers commands

// Prevents additional console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use sapo_printer::infrastructure::database::{
    run_migrations, DbPool, SqliteEventStore, SqlitePrintJobRepository, SqlitePrinterRepository,
};
use sapo_printer::infrastructure::downloader::ReqwestDownloader;
use sapo_printer::infrastructure::eventbus::tauri_event_bus::TauriEventBus;
use sapo_printer::infrastructure::printer::PrinterManager;
use sapo_printer::infrastructure::queue::{QueueWorker, SqliteQueueManager};
use sapo_printer::infrastructure::secrets::SecretManager;
use sapo_printer::interface::tauri::dtos::printer_dto::{
    PrinterConfigDto, PrinterDto, PrinterStatusDto,
};
use sapo_printer::shared::event_bus::EventBus;
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

/// List all available printers (discovered + saved configs)
#[tauri::command]
fn list_printers(app_ctx: tauri::State<AppContextState>) -> Result<Vec<PrinterDto>, String> {
    // 1. Discover printers from OS
    let discovered = app_ctx.printer_manager.discover_printers();

    // 2. Load saved configs from repository
    let saved_printers = app_ctx
        .printer_repo
        .find_all()
        .map_err(|e| format!("Không thể tải cấu hình máy in: {}", e))?;

    // 3. Map to DTOs - merge discovered with saved configs
    let dtos: Vec<PrinterDto> = discovered
        .iter()
        .map(|printer| {
            let printer_name = printer.name().as_str();

            // Check if this printer has saved config with is_default flag
            let is_default = saved_printers
                .iter()
                .find(|saved| saved.name().as_str() == printer_name)
                .and({
                    // TODO: Once Printer aggregate includes is_default field, use it here
                    // For now, return None since domain model doesn't expose is_default yet
                    None
                });

            PrinterDto {
                name: printer_name.to_string(),
                device_id: printer_name.to_string(), // device_id = printer_name
                status: match printer.status() {
                    sapo_printer::domain::printer::PrinterStatus::Online => "Online".to_string(),
                    sapo_printer::domain::printer::PrinterStatus::Offline => "Offline".to_string(),
                    sapo_printer::domain::printer::PrinterStatus::Error => "Error".to_string(),
                },
                printer_type: match printer.printer_type() {
                    sapo_printer::domain::printer::PrinterType::Local => "Local".to_string(),
                    sapo_printer::domain::printer::PrinterType::Network => "Network".to_string(),
                },
                is_default,
            }
        })
        .collect();

    Ok(dtos)
}

/// Save printer configuration
#[tauri::command]
fn save_printer_config(
    config: PrinterConfigDto,
    app_ctx: tauri::State<AppContextState>,
) -> Result<(), String> {
    // 1. Validate config fields
    // Paper size validation
    if config.paper_size.is_empty() {
        return Err("Khổ giấy không được để trống".to_string());
    }

    // Custom dimensions validation (50-500mm range)
    if config.paper_size == "Custom" {
        if let Some(width) = config.paper_width {
            if !(50..=500).contains(&width) {
                return Err("Chiều rộng giấy phải trong khoảng 50-500mm".to_string());
            }
        } else {
            return Err("Chiều rộng giấy bắt buộc khi chọn khổ Custom".to_string());
        }

        if let Some(height) = config.paper_height {
            if !(50..=500).contains(&height) {
                return Err("Chiều cao giấy phải trong khoảng 50-500mm".to_string());
            }
        } else {
            return Err("Chiều cao giấy bắt buộc khi chọn khổ Custom".to_string());
        }
    } else {
        // Non-Custom paper size should not have dimensions
        if config.paper_width.is_some() || config.paper_height.is_some() {
            return Err(
                "Không được cung cấp kích thước tùy chỉnh khi chọn khổ giấy chuẩn".to_string(),
            );
        }
    }

    // Margins validation (0-100mm range, guard against u32 overflow)
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

    // 2. Load or create Printer from repository
    use sapo_printer::domain::printer::{PrinterName, PrinterType};

    let printer_name = PrinterName::new(config.printer_name.clone());
    let printer = match app_ctx
        .printer_repo
        .find_by_name(&printer_name)
        .map_err(|e| format!("Không thể tải cấu hình máy in: {}", e))?
    {
        Some(p) => p,
        None => {
            // Create new printer if not found
            sapo_printer::domain::printer::Printer::new(printer_name, PrinterType::Local)
        }
    };

    // 3. Note: Config fields (paper size, margins, etc.) are stored in printer_configs table
    //    but are NOT part of the Printer aggregate domain model. The repository handles
    //    these separately. We just save the Printer aggregate.

    // 4. Save printer (repository will handle config fields via UPSERT)
    app_ctx
        .printer_repo
        .save(&printer)
        .map_err(|e| format!("Không thể lưu cấu hình máy in: {}", e))?;

    // TODO: In future, extend repository to accept config parameters
    // For now, the basic save() works because repository has default values

    Ok(())
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

fn main() {
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
            let printer_repo = Arc::new(SqlitePrinterRepository::new(pool.get_arc()));
            let job_repo = Arc::new(SqlitePrintJobRepository::new(pool.get_arc()));
            let event_store = Arc::new(SqliteEventStore::new(pool.get_arc()));
            let event_bus: Arc<dyn EventBus> =
                Arc::new(TauriEventBus::new(app_handle.clone()));
            let queue_manager: Arc<dyn sapo_printer::infrastructure::queue::QueueManager> =
                Arc::new(SqliteQueueManager::new(pool.get_arc()));

            #[cfg(target_os = "windows")]
            let printer_manager: Arc<dyn PrinterManager> = Arc::new(Win32PrinterManager::new());

            #[cfg(not(target_os = "windows"))]
            let printer_manager: Arc<dyn PrinterManager> = Arc::new(CupsPrinterManager::new());

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

            // Register managed state
            app.manage(AppContextState {
                printer_repo,
                printer_manager,
                _secret_manager: secret_manager,
                job_repo,
                event_store,
                event_bus,
                queue_manager,
                queue_worker: worker,
                app_handle,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_printers,
            save_printer_config,
            get_printer_status,
            create_print_job,
            cancel_print_job,
            list_jobs,
            get_job_status,
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
