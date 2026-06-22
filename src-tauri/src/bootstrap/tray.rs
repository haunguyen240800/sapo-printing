use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{App, Manager};

use crate::AppContextState;

/// Tạo system tray icon với menu "Hiển thị / Thoát".
/// Nếu app được khởi động với `--minimized`, ẩn cửa sổ main ngay sau đó.
pub fn setup_tray(app: &App) -> tauri::Result<()> {
    let show_item = MenuItemBuilder::with_id("show", "Hiển thị").build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Thoát").build(app)?;
    let tray_menu = MenuBuilder::new(app)
        .item(&show_item)
        .separator()
        .item(&quit_item)
        .build()?;

    let mut tray_builder = TrayIconBuilder::with_id("main-tray")
        .tooltip(env!("SAPO_PRODUCT_NAME"))
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
                let state = app.state::<AppContextState>();
                if let Err(e) = state.queue_worker.stop() {
                    eprintln!("Warning: Failed to stop queue worker gracefully: {e}");
                }
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
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

    // Ẩn cửa sổ nếu khởi động ở chế độ minimized (e.g. autostart)
    if std::env::args().any(|arg| arg == "--minimized") {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }
    }

    Ok(())
}
