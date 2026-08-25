use tauri::{AppHandle, Emitter, Manager};

use crate::infrastructure::platform::updater::update_checker;
use crate::{AppContextState, application::models::UpdateCheckResponse};

/// Đăng ký tauri-plugin-updater và spawn background task kiểm tra update.
/// - Lần đầu: kiểm tra ngay khi startup.
/// - Sau đó: lặp lại mỗi 24 giờ.
/// Emit `update-available` về frontend nếu có version mới (dedup theo version string).
#[cfg(desktop)]
pub fn setup_updater(app: &tauri::App) -> tauri::Result<()> {
    app.handle()
        .plugin(tauri_plugin_updater::Builder::new().build())?;

    let handle = app.handle().clone();
    tauri::async_runtime::spawn(run_update_loop(handle));

    Ok(())
}

#[cfg(desktop)]
async fn run_update_loop(handle: AppHandle) {
    // Kiểm tra ngay lúc khởi động
    check_and_emit(&handle).await;

    // Lặp lại mỗi 24 giờ
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(24 * 60 * 60)).await;
        check_and_emit(&handle).await;
    }
}

#[cfg(desktop)]
async fn check_and_emit(handle: &AppHandle) {
    match update_checker::check_for_updates(handle).await {
        Ok(result) if result.update_available => {
            tracing::info!(
                target = "sapo_printer::updater",
                version = ?result.version,
                "Update available"
            );

            let state = handle.state::<AppContextState>();
            let mut last = state.last_emitted_update_version.lock().unwrap();

            if result.version != *last {
                let dto = UpdateCheckResponse {
                    update_available: result.update_available,
                    version: result.version.clone(),
                    release_notes: result.release_notes,
                };
                let _ = handle.emit("update-available", &dto);
                *last = result.version;
            }
        }
        Ok(_) => {
            tracing::info!(target = "sapo_printer::updater", "No update available");
        }
        Err(e) => {
            tracing::warn!(
                target = "sapo_printer::updater",
                error = %e,
                "Update check failed (non-fatal)"
            );
        }
    }
}
