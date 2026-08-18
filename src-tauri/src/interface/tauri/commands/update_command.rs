use tauri::{AppHandle, Emitter, State};

use crate::AppContextState;
use crate::application::models::UpdateCheckResponse;
use crate::infrastructure::platform::updater::{agent_client, update_checker};

/// Thời gian chờ trước khi app tự thoát để service (SYSTEM) ghi đè file khi cài.
const AGENT_EXIT_DELAY: std::time::Duration = std::time::Duration::from_secs(3);

/// Check whether an application update is available.
#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<UpdateCheckResponse, String> {
    let result = update_checker::check_for_updates(&app).await?;
    Ok(UpdateCheckResponse {
        update_available: result.update_available,
        version: result.version,
        release_notes: result.release_notes,
    })
}

/// Download and install the available update.
///
/// Đường chính: uỷ quyền cho `sapo-printer-cert-manager` (SYSTEM) cài im lặng (không UAC).
/// Fallback: nếu service không phản hồi, tự dùng tauri-updater (có UAC) để không chặn update.
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    ctx: State<'_, AppContextState>,
) -> Result<(), String> {
    let _guard = ctx
        .install_guard
        .try_acquire()
        .ok_or_else(|| "An update is already being installed".to_string())?;

    // Xác định version mới nhất để đối chiếu với service.
    let check = update_checker::check_for_updates(&app).await?;
    let expected_version = check
        .version
        .filter(|_| check.update_available)
        .ok_or_else(|| "No update available".to_string())?;

    // Reset dedup so periodic check re-notifies if user doesn't restart.
    if let Ok(mut version) = ctx.last_emitted_update_version.lock() {
        *version = None;
    }

    let app_pid = std::process::id();
    match agent_client::request_update(expected_version, app_pid).await {
        Ok(()) => {
            // Service đã staged bản mới và sẽ cài sau khi app thoát → tự thoát sau ít giây.
            tracing::info!(
                target = "sapo_printer::updater",
                "Update delegated to agent (silent, no UAC)"
            );
            let _ = app.emit("update-installing-agent", ());
            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(AGENT_EXIT_DELAY).await;
                handle.exit(0);
            });
            Ok(())
        }
        Err(e) => {
            // Service không có/không phản hồi → fallback đường UAC cũ để vẫn update được.
            tracing::warn!(
                target = "sapo_printer::updater",
                error = %e,
                "Agent unavailable, falling back to UAC updater"
            );
            update_checker::download_and_install_update(&app).await?;
            let _ = app.emit("update-ready-to-apply", ());
            Ok(())
        }
    }
}

/// Restart the application (used after an update is installed).
#[tauri::command]
pub fn restart_app() -> Result<(), String> {
    std::process::exit(0);
}

/// Thoát hẳn app — dùng cho nhánh forced update khi user chọn "Thoát" sau khi update thất bại.
#[tauri::command]
pub fn quit_app(app: AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}
