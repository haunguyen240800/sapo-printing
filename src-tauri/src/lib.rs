pub mod application;
pub mod bootstrap;
pub mod domain;
pub mod infrastructure;
pub mod interface;

use application::use_cases::{
    CancelPrintJobUseCase, ClearHistoryUseCase, CreatePrintJobUseCase, GetAuditTrailUseCase,
    GetJobStatusUseCase, GetMetricsUseCase, ListPrintersUseCase,
};
use std::sync::Arc;

pub struct AppContextState {
    pub create_print_job_uc: Arc<CreatePrintJobUseCase>,
    pub get_job_status_uc: Arc<GetJobStatusUseCase>,
    pub cancel_print_job_uc: Arc<CancelPrintJobUseCase>,
    pub get_metrics_uc: Arc<GetMetricsUseCase>,
    pub get_audit_trail_uc: Arc<GetAuditTrailUseCase>,
    pub list_printers_uc: Arc<ListPrintersUseCase>,
    pub clear_history_uc: Arc<ClearHistoryUseCase>,
    pub queue_worker: Arc<infrastructure::worker::QueueWorker>,
    pub app_handle: tauri::AppHandle,
    pub print_config_path: std::path::PathBuf,
    pub install_guard: infrastructure::platform::updater::update_checker::InstallGuard,
    pub last_emitted_update_version: std::sync::Mutex<Option<String>>,
    /// Bản đã tải bằng Tauri updater, chờ user xác nhận mở installer (handle + bytes).
    pub pending_update: std::sync::Mutex<Option<(tauri_plugin_updater::Update, Vec<u8>)>>,
}

pub fn run() {
    use infrastructure::telemetry::logger::init_logging;
    use interface::tauri::commands::{
        auth_command::approve_pairing_request,
        autostart_command::{get_autostart_enabled, set_autostart_enabled},
        clear_command::clear_job_history,
        metrics_command::get_metrics,
        printer_command::{
            detect_printer_category, get_printer_config, get_printer_status, list_printers,
            save_printer_config,
        },
        update_command::{check_for_updates, install_update, quit_app, restart_app},
    };
    use tauri::Manager;

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .setup(move |app| {
            // Resolve OS-standard data/log dirs, then initialize logging first so
            // subsequent startup steps are captured.
            let dirs = bootstrap::dirs::init_directories(app.handle());
            init_logging(&dirs.log_dir);

            #[cfg(target_os = "windows")]
            setup_windows_titlebar(app);

            let pool = bootstrap::database::init_database(&dirs.db_path_str, &dirs.temp_dir);

            let resource_dir = app.path().resource_dir().ok();
            let state = bootstrap::app_state::build_app_state(
                pool.clone(),
                &dirs.temp_dir,
                &dirs.print_config_path,
                app.handle().clone(),
                resource_dir,
            );
            app.manage(state);

            bootstrap::http_server::start_http_server(app, pool, &dirs.data_dir);

            app.handle().plugin(tauri_plugin_dialog::init())?;
            app.handle().plugin(tauri_plugin_opener::init())?;

            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                Some(vec!["--minimized"]),
            ))?;

            bootstrap::tray::setup_tray(app)?;

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
            quit_app,
            get_autostart_enabled,
            set_autostart_enabled,
            approve_pairing_request,
            clear_job_history,
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
