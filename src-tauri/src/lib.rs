// Library root — declares 4-layer Clean Architecture modules + bootstrap

pub mod bootstrap;

// Interface Layer
pub mod interface;

// Application Layer
pub mod application;

// Domain Layer
pub mod domain;

// Infrastructure Layer
pub mod infrastructure;

use std::sync::Arc;
use application::use_cases::{
    CreatePrintJobUseCase, GetAuditTrailUseCase,
    GetJobStatusUseCase, GetMetricsUseCase, ListPrintersUseCase,
};

/// Shared state registered with Tauri via `.manage()`.
/// Commands access this via `tauri::State<'_, AppContextState>`.
pub struct AppContextState {
    pub create_print_job_uc: Arc<CreatePrintJobUseCase>,
    pub get_job_status_uc: Arc<GetJobStatusUseCase>,
    pub get_metrics_uc: Arc<GetMetricsUseCase>,
    pub get_audit_trail_uc: Arc<GetAuditTrailUseCase>,
    pub list_printers_uc: Arc<ListPrintersUseCase>,
    pub queue_worker: Arc<infrastructure::worker::QueueWorker>,
    pub app_handle: tauri::AppHandle,
    pub install_guard: infrastructure::platform::updater::update_checker::InstallGuard,
    pub last_emitted_update_version: std::sync::Mutex<Option<String>>,
}

pub fn run() {
    use infrastructure::telemetry::logger::init_logging;
    use tauri::Manager;
    use interface::tauri::commands::{
        autostart_command::{get_autostart_enabled, set_autostart_enabled},
        auth_command::approve_pairing_request,
        metrics_command::get_metrics,
        printer_command::{
            detect_printer_category, get_printer_config, get_printer_status,
            list_printers, save_printer_config,
        },
        update_command::{check_for_updates, install_update, restart_app},
    };

    init_logging();

    let dirs = bootstrap::dirs::init_directories();

    tauri::Builder::default()
        .setup(move |app| {
            // 1. Windows title-bar colour (platform-specific)
            #[cfg(target_os = "windows")]
            setup_windows_titlebar(app);

            // 2. Database + migrations + temp-file cleanup
            let pool =
                bootstrap::database::init_database(&dirs.db_path_str, &dirs.temp_dir);

            // 3. Dependency injection — wires all ports, starts queue worker
            let resource_dir = app.path().resource_dir().ok();
            let state = bootstrap::app_state::build_app_state(
                pool.clone(),
                &dirs.temp_dir,
                app.handle().clone(),
                resource_dir,
            );
            app.manage(state);

            // 4. HTTPS agent server (background task)
            bootstrap::http_server::start_http_server(app, pool, &dirs.data_dir);

            // 5. Tauri plugins
            app.handle().plugin(tauri_plugin_dialog::init())?;

            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                Some(vec!["--minimized"]),
            ))?;

            // 6. System tray
            bootstrap::tray::setup_tray(app)?;

            // 7. Auto-updater plugin + background check loop
            #[cfg(desktop)]
            bootstrap::updater::setup_updater(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_printers,
            save_printer_config,
            get_printer_config,
            get_printer_status,
            detect_printer_category,
            get_metrics,
            check_for_updates,
            install_update,
            restart_app,
            get_autostart_enabled,
            set_autostart_enabled,
            approve_pairing_request,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("Failed to start Tauri application: {e}");
            std::process::exit(1);
        });
}

/// Đặt màu title bar trên Windows để khớp với theme của app.
#[cfg(target_os = "windows")]
fn setup_windows_titlebar(app: &tauri::App) {
    use tauri::Manager;
    use windows::Win32::Foundation::COLORREF;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DwmSetWindowAttribute,
    };

    if let Some(window) = app.get_webview_window("main") {
        if let Ok(raw) = window.hwnd() {
            let hwnd = HWND(raw.0 as isize);
            let caption = COLORREF(0x00FFFFFF);
            let text = COLORREF(0x00000000);
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
