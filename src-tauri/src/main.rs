// Entry point for Tauri application
// This file initializes the Tauri runtime and registers commands

// Prevents additional console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use sapo_printer::AppContextState;
use sapo_printer::application::models::{
    PrinterConfigRequest, PrinterConfigResponse, PrinterResponse, PrinterStatusResponse,
};
use sapo_printer::application::ports::event_bus::EventBus;
use sapo_printer::application::ports::{
    ConfigPort, EventStore, MetricsPort, PrinterPort, QueuePort, SecretPort, TempFilePort,
};

use sapo_printer::infrastructure::configs::app::JsonFileConfigProvider;
use sapo_printer::infrastructure::configs::db::{DbPool, run_migrations};
use sapo_printer::infrastructure::integrations::network::ReqwestDownloader;
use sapo_printer::infrastructure::integrations::pdf_engine::bitmap_strategy::BitmapRenderStrategy;
use sapo_printer::infrastructure::integrations::pdf_engine::pdfium_loader;
use sapo_printer::infrastructure::persistence::JobQueueBroker;
use sapo_printer::infrastructure::persistence::{EventRepository, PrintJobRepository};
use sapo_printer::infrastructure::platform::printer_api::SystemPrinterManager;
use sapo_printer::infrastructure::platform::printing::DefaultPrintService;
use sapo_printer::infrastructure::telemetry::logger::init_logging;
use sapo_printer::infrastructure::telemetry::metrics::MetricsCollector;
use sapo_printer::infrastructure::temp_file::{self, FilesystemTempFileManager};
use sapo_printer::infrastructure::worker::QueueWorker;
use sapo_printer::interface::tauri::print_job_event_emitter::PrintJobEventEmitter;
use std::sync::Arc;
use tauri::Manager;

/// Tauri command: kiểm tra app có đang bật khởi động cùng OS hay không.
#[tauri::command]
fn get_autostart_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

/// Tauri command: bật/tắt khởi động cùng OS.
#[tauri::command]
fn set_autostart_enabled(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())
    } else {
        manager.disable().map_err(|e| e.to_string())
    }
}

#[cfg(target_os = "windows")]
use sapo_printer::infrastructure::platform::keychain::WindowsCredentialManager;

#[cfg(target_os = "macos")]
use sapo_printer::infrastructure::platform::keychain::MacOSKeychain;

#[cfg(target_os = "linux")]
use sapo_printer::infrastructure::platform::keychain::LinuxSecretService;

#[tauri::command]
fn list_printers(ctx: tauri::State<'_, AppContextState>) -> Result<Vec<PrinterResponse>, String> {
    ctx.list_printers_uc
        .clone()
        .execute()
        .map_err(|e| format!("{:?}", e))
}

/// Save printer configuration
#[tauri::command]
fn save_printer_config(
    config: PrinterConfigRequest,
    _app_ctx: tauri::State<'_, AppContextState>,
) -> Result<(), String> {
    use sapo_printer::infrastructure::configs::app::app_print_config;

    // 1. Validate config fields
    // Paper size validation
    if config.paper_size.is_empty() {
        return Err("Khá»• giáº¥y khÃ´ng Ä‘Æ°á»£c Ä‘á»ƒ trá»‘ng".to_string());
    }

    // Dimensions validation (50-500mm range when provided)
    if config.paper_width > 0 && !(50..=500).contains(&config.paper_width) {
        return Err("Chiá» u rá»™ng giáº¥y pháº£i trong khoáº£ng 50-500mm".to_string());
    }

    if config.paper_height > 0 && !(50..=500).contains(&config.paper_height) {
        return Err("Chiá» u cao giáº¥y pháº£i trong khoáº£ng 50-500mm".to_string());
    }

    // For Custom paper size, dimensions are required
    if config.paper_size == "Custom" {
        if config.paper_width == 0 {
            return Err("Chiá» u rá»™ng giáº¥y báº¯t buá»™c khi chá» n khá»• Custom".to_string());
        }
        if config.paper_height == 0 {
            return Err("Chiá» u cao giáº¥y báº¯t buá»™c khi chá» n khá»• Custom".to_string());
        }
    }

    // Margins validation (0-100mm range)
    if config.margin_left > 100 {
        return Err("Lá» trÃ¡i pháº£i trong khoáº£ng 0-100mm".to_string());
    }
    if config.margin_right > 100 {
        return Err("Lá» pháº£i pháº£i trong khoáº£ng 0-100mm".to_string());
    }
    if config.margin_top > 100 {
        return Err("Lá» trÃªn pháº£i trong khoáº£ng 0-100mm".to_string());
    }
    if config.margin_bottom > 100 {
        return Err("Lá» dÆ°á»›i pháº£i trong khoáº£ng 0-100mm".to_string());
    }

    // Printer name validation
    if config.printer_name.is_empty() {
        return Err("TÃªn mÃ¡y in khÃ´ng Ä‘Æ°á»£c Ä‘á»ƒ trá»‘ng".to_string());
    }

    // Buffer validation
    if config.enable_buffer {
        if config.buffer_size_kb == 0 {
            return Err("KÃ­ch thÆ°á»›c buffer báº¯t buá»™c khi báº­t buffer".to_string());
        }
        if !(1..=1024).contains(&config.buffer_size_kb) {
            return Err(format!(
                "KÃ­ch thÆ°á»›c buffer pháº£i trong khoáº£ng 1-1024 KB (nháº­n Ä‘Æ°á»£c: {} KB)",
                config.buffer_size_kb
            ));
        }
    } else if config.buffer_size_kb > 0 {
        return Err("KhÃ´ng thá»ƒ Ä‘áº·t kÃ­ch thÆ°á»›c buffer khi buffer Ä‘Ã£ táº¯t".to_string());
    }

    // Color mode validation
    let valid_color_modes = ["RGB", "ARGB", "BGR", "GRAY", "BINARY"];
    if !valid_color_modes.contains(&config.color_mode.as_str()) {
        return Err(format!(
            "Loáº¡i áº£nh in khÃ´ng há»£p lá»‡: '{}'. Chá»‰ cháº¥p nháº­n: RGB, ARGB, BGR, GRAY, BINARY",
            config.color_mode
        ));
    }

    // 2. Convert DTO to config store model
    let print_config = app_print_config::AppPrintConfig {
        printer_name: config.printer_name,
        paper_size: config.paper_size,
        paper_width: Some(config.paper_width),
        paper_height: Some(config.paper_height),
        orientation: config.orientation,
        margin_left: config.margin_left,
        margin_right: config.margin_right,
        margin_top: config.margin_top,
        margin_bottom: config.margin_bottom,
        color_mode: config.color_mode,
        print_as_image: config.print_as_image,
        enable_buffer: config.enable_buffer,
        buffer_size_kb: Some(config.buffer_size_kb),
    };

    // 3. Save to JSON file
    app_print_config::save_config(&print_config)?;

    Ok(())
}

/// Get printer configuration (global app config)
#[tauri::command]
fn get_printer_config(
    _app_ctx: tauri::State<'_, AppContextState>,
) -> Result<PrinterConfigResponse, String> {
    use sapo_printer::infrastructure::configs::app::app_print_config;

    let config = app_print_config::load_config()?;

    match config {
        Some(cfg) => Ok(PrinterConfigResponse {
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
        None => Ok(PrinterConfigResponse {
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
fn get_printer_status(_name: String) -> Result<PrinterStatusResponse, String> {
    Ok(PrinterStatusResponse {
        status: "Online".to_string(),
    })
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
    let db_path_str = db_path
        .to_str()
        .unwrap_or_else(|| {
            eprintln!("Database path contains non-UTF-8 characters");
            std::process::exit(1);
        })
        .to_string();

    tauri::Builder::default()
        .setup(move |app| {
            let app_handle = app.handle().clone();

            // ==================== TITLE BAR COLOR ====================
            // Đổi màu nền + màu chữ của title bar native (Windows 11 build 22000+)
            // qua DWM. Windows 10 sẽ bỏ qua (giữ màu theo theme hệ thống).
            #[cfg(target_os = "windows")]
            {
                use tauri::Manager;
                use windows::Win32::Foundation::{COLORREF, HWND};
                use windows::Win32::Graphics::Dwm::{DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
                                                    DwmSetWindowAttribute,
                };

                if let Some(window) = app.get_webview_window("main") {
                    if let Ok(raw) = window.hwnd() {
                        let hwnd = HWND(raw.0 as isize);
                        let caption = COLORREF(0x00FFFFFF); // nền: #0088ff
                        let text = COLORREF(0x00000000); // chữ: #ffffff
                        let size = size_of::<COLORREF>() as u32;
                        unsafe {
                            let _ = DwmSetWindowAttribute(
                                hwnd,
                                DWMWA_CAPTION_COLOR,
                                &caption as *const _ as *const _,
                                size,
                            );
                            let _ = DwmSetWindowAttribute(
                                hwnd,
                                DWMWA_TEXT_COLOR,
                                &text as *const _ as *const _,
                                size,
                            );
                        }
                    }
                }
            }

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
            // migration 2 â€” `temp_file_retention_hours`), falling back to the
            // hardcoded default on any read failure.
            let retention = temp_file::load_retention(&pool);
            temp_file::startup_cleanup(&temp_dir, retention);

            // Initialize AppContext dependencies
            let job_repo = Arc::new(PrintJobRepository::new(pool.clone()));

            // Initialize secret manager
            #[cfg(target_os = "windows")]
            let secret_manager: Arc<dyn SecretPort> =
                Arc::new(WindowsCredentialManager::new());

            #[cfg(target_os = "macos")]
            let secret_manager: Arc<dyn SecretPort> = Arc::new(MacOSKeychain::new());

            #[cfg(target_os = "linux")]
            let secret_manager: Arc<dyn SecretPort> = Arc::new(
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

            let event_store = Arc::new(EventRepository::new(pool.clone(), secret_manager.clone()));

            // EventBus is just a pub/sub dispatcher (handlers run synchronously
            // in the publisher thread). UI emission is a SEPARATE subscriber:
            // `PrintJobEventEmitter` lives in the interface layer.
            let event_bus: Arc<dyn EventBus> =
                Arc::new(sapo_printer::infrastructure::eventbus::InMemoryEventBus::new());

            // It bridges Domain events to Tauri UI events.
            // Note: We don't store it in AppContextState, we just register it
            // directly with the global `event_bus`.
            PrintJobEventEmitter::new(app_handle.clone()).register(&event_bus);

            let queue_manager: Arc<dyn QueuePort> =
                Arc::new(JobQueueBroker::new(pool.clone()));

            // Register PrintJobCreatedHandler to listen for PrintJobCreated events
            let push_handler = Arc::new(sapo_printer::application::handlers::print_job_created_handler::PrintJobCreatedHandler::new(
                queue_manager.clone(),
            ));
            event_bus.subscribe("PrintJobCreated", push_handler);
            tracing::info!(
                target = "sapo_printer::startup",
                "PrintJobCreatedHandler registered for PrintJobCreated events"
            );


            // Initialize QueueWorker dependencies
            let downloader: Arc<dyn sapo_printer::application::ports::DownloadPort> =
                Arc::new(ReqwestDownloader::new());
            let render_strategy: Arc<dyn sapo_printer::infrastructure::integrations::pdf_engine::renderer::RenderStrategy> =
                Arc::new(BitmapRenderStrategy::new().map_err(|e| e.to_string())?);
            let print_service: Arc<dyn sapo_printer::application::ports::PrintPort> =
                Arc::new(DefaultPrintService::new(render_strategy));

            // Filesystem temp file manager (owns the per-app temp directory)
            let temp_files: Arc<dyn TempFilePort> =
                Arc::new(FilesystemTempFileManager::new(temp_dir.clone()));

            // OS-backed PrinterPort â€” used both for ONLINE availability checks
            // and for the `list_printers` Tauri command.
            let printer_manager: Arc<dyn PrinterPort> =
                Arc::new(SystemPrinterManager::new());

            // Config provider â€” JSON file backed
            let config_provider: Arc<dyn ConfigPort> =
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
                sapo_printer::application::handlers::print_job_failed_handler::PrintJobFailedHandler::new(
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

            // Create MetricsCollector (concrete) and cast to MetricsPort port
            let metrics_provider: Arc<dyn MetricsPort> = Arc::new(MetricsCollector::new(
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

            // ==================== Composition root: build use cases ====================
            // The interface layer (Tauri commands + HTTP handlers) invokes these
            // pre-built use cases and never touches Domain repositories or
            // Infrastructure ports directly.
            use sapo_printer::application::use_cases::{
                CreatePrintJobUseCase, GetAuditTrailUseCase,
                GetJobStatusUseCase, GetMetricsUseCase, ListPrintersUseCase,
            };

            let create_print_job_uc = Arc::new(CreatePrintJobUseCase {
                job_repo: Arc::clone(&job_repo_port),
                event_store: Arc::clone(&event_store_port),
                event_bus: Arc::clone(&event_bus),
                config_provider: Arc::clone(&config_provider),
                printer_manager: Arc::clone(&printer_manager),
            });

            let get_job_status_uc =
                Arc::new(GetJobStatusUseCase::new(Arc::clone(&job_repo_port)));
            let get_metrics_uc =
                Arc::new(GetMetricsUseCase::new(Arc::clone(&metrics_provider)));
            let get_audit_trail_uc = Arc::new(GetAuditTrailUseCase::new(
                Arc::clone(&event_store_port),
                Arc::clone(&secret_manager),
            ));
            let list_printers_uc =
                Arc::new(ListPrintersUseCase::new(Arc::clone(&printer_manager)));

            // Register managed state
            app.manage(AppContextState {
                create_print_job_uc,

                get_job_status_uc,
                get_metrics_uc,
                get_audit_trail_uc,
                list_printers_uc,
                queue_worker: worker,
                app_handle,
                install_guard: sapo_printer::infrastructure::platform::updater::update_checker::InstallGuard::new(),
                last_emitted_update_version: std::sync::Mutex::new(None),
            });

            // ==================== HTTPS local server bootstrap ====================
            // Requires helper service `sapo-printer-agent` Ä‘Ã£ sinh cert vÃ o data_dir/tls/.
            {
                use sapo_printer::interface::http_server;
                use sapo_printer::interface::tauri::commands::auth_command::AgentState;

                let bootstrap_data_dir = data_dir.clone();
                // Cert dÃ¹ng chung vá»›i cert-manager (ProgramData\SapoPrinter trÃªn Windows).
                // App Ä‘á»c cert Ä‘Ã£ Ä‘Æ°á»£c installer provision + trust, khÃ´ng tá»± cÃ i CA.
                let bootstrap_cert_dir =
                    sapo_printer::infrastructure::platform::tls::shared_cert_dir();
                // Táº¡o ApiTokenRepository táº¡i composition root, truyá»n vÃ o bootstrap dÆ°á»›i dáº¡ng
                // trait object â€” interface layer khÃ´ng phá»¥ thuá»™c vÃ o infrastructure::persistence.
                let bootstrap_token_manager: Arc<dyn sapo_printer::application::ports::ApiTokenPort> =
                    sapo_printer::infrastructure::persistence::ApiTokenRepository::new(pool.clone());
                let bootstrap_ctx = app.state::<AppContextState>();
                let bootstrap_event_bus = event_bus.clone();
                let bootstrap_create_uc = bootstrap_ctx.create_print_job_uc.clone();
                let bootstrap_get_status_uc = bootstrap_ctx.get_job_status_uc.clone();
                let app_handle_for_agent = app.handle().clone();

                tauri::async_runtime::spawn(async move {
                    match http_server::start_bootstrap(
                        &bootstrap_cert_dir,
                        &bootstrap_data_dir,
                        bootstrap_token_manager,
                        bootstrap_event_bus,
                        bootstrap_create_uc,
                        bootstrap_get_status_uc,
                        env!("CARGO_PKG_VERSION"),
                    )
                        .await
                    {
                        Ok(result) => {
                            tracing::info!(port = result.port, "HTTPS agent started");
                            // Wire pair request â†’ Tauri emit â†’ front-end toast.
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
                            tracing::error!(error = %e, "HTTPS agent bootstrap failed â€” webapp integration disabled");
                        }
                    }
                });
            }

            // Register dialog plugin for native file dialogs
            app.handle().plugin(tauri_plugin_dialog::init())?;

            // Register autostart plugin (khởi động cùng OS). Mặc định KHÔNG bật;
            // user tự bật/tắt trong Settings qua command get/set_autostart_enabled.
            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                Some(vec!["--minimized"]),
            ))?;

            // ==================== SYSTEM TRAY ====================
            // Cho phép app chạy ngầm khi đóng cửa sổ. Bấm X sẽ ẩn xuống tray
            // (xem on_window_event), thoát hẳn qua menu "Thoát" của tray.
            {
                use tauri::Manager;
                use tauri::menu::{MenuBuilder, MenuItemBuilder};
                use tauri::tray::{TrayIconBuilder, TrayIconEvent};

                let show_item = MenuItemBuilder::with_id("show", "Hiển thị").build(app)?;
                let quit_item = MenuItemBuilder::with_id("quit", "Thoát").build(app)?;
                let tray_menu = MenuBuilder::new(app)
                    .item(&show_item)
                    .separator()
                    .item(&quit_item)
                    .build()?;

                let mut tray_builder = TrayIconBuilder::with_id("main-tray")
                    .tooltip("Sapo Printer Pro Max")
                    .menu(&tray_menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(|app, event| match event.id().as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.unminimize();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            // Dừng queue worker gracefully trước khi thoát hẳn.
                            let state = app.state::<AppContextState>();
                            if let Err(e) = state.queue_worker.stop() {
                                eprintln!("Warning: Failed to stop queue worker gracefully: {e}");
                            }
                            app.exit(0);
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        // Click trái vào icon -> hiện lại cửa sổ.
                        if let TrayIconEvent::Click {
                            button: tauri::tray::MouseButton::Left,
                            button_state: tauri::tray::MouseButtonState::Up,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.unminimize();
                                let _ = window.set_focus();
                            }
                        }
                    });

                if let Some(icon) = app.default_window_icon().cloned() {
                    tray_builder = tray_builder.icon(icon);
                }

                tray_builder.build(app)?;

                // Khi được auto-start cùng OS (cờ --minimized), khởi động ẩn
                // xuống tray để chạy hoàn toàn ngầm.
                if std::env::args().any(|arg| arg == "--minimized") {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.hide();
                    }
                }
            }

            // ==================== AUTO-UPDATE ====================
            // CÆ¡ cháº¿ check version / auto-update qua tauri-plugin-updater.
            // NOTE(local-publish): viá»‡c phÃ¡t hÃ nh `latest.json` á»Ÿ LOCAL Ä‘Ã£ bá»‹ gá»¡
            // (`scripts/update-latest-json.mjs` + hook trong `pnpm build` + file
            // `installers/latest.json`). Sáº½ thay báº±ng CI phÃ¡t hÃ nh theo git tag `v*`
            // (GitLab CI/CD) tá»± sinh + upload `latest.json` nhÆ° release artifact.
            // Endpoint trong `tauri.conf.json` cáº§n trá» vá» GitLab release sau nÃ y.

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
                    use sapo_printer::application::models::UpdateCheckResponse;
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
                            let state = update_handle.state::<AppContextState>();
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
                                let state = update_handle.state::<AppContextState>();
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

            sapo_printer::interface::tauri::commands::metrics_command::get_metrics,
            sapo_printer::interface::tauri::commands::update_command::check_for_updates,
            sapo_printer::interface::tauri::commands::update_command::install_update,
            sapo_printer::interface::tauri::commands::update_command::restart_app,
            get_autostart_enabled,
            set_autostart_enabled,
            sapo_printer::interface::tauri::commands::auth_command::approve_pairing_request,
        ])
        .on_window_event(|window, event| {
            // Bấm X không thoát app mà thu nhỏ xuống system tray; queue worker
            // vẫn chạy ngầm để tiếp tục xử lý job in. Thoát hẳn qua menu "Thoát"
            // của tray (nơi worker được dừng gracefully trước khi app.exit).
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("Failed to start Tauri application:");
            eprintln!("  {e}");
            std::process::exit(1);
        });
}


