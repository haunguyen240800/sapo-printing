use tauri::{AppHandle, Emitter, State};

use crate::AppContextState;
use crate::application::models::UpdateCheckResponse;
use crate::infrastructure::platform::updater::{agent_client, update_checker};

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
            // Service đã staged bản mới; nó sẽ cài + relaunch NGAY SAU KHI app này thoát.
            // Không tự thoát — báo frontend hiện nút để user chủ động bấm "Khởi động lại".
            tracing::info!(
                target = "sapo_printer::updater",
                "Update staged by agent (silent, no UAC) — waiting for user restart"
            );
            let _ = app.emit("update-ready-to-apply", ());
            Ok(())
        }
        Err(e) => {
            // Service không có/không phản hồi → fallback đường UAC: TẢI trước (app vẫn chạy),
            // lưu lại chờ user bấm "Khởi động lại" mới cài (installer sẽ đóng + relaunch app).
            tracing::warn!(
                target = "sapo_printer::updater",
                error = %e,
                "Agent unavailable, falling back to UAC updater (download only)"
            );
            let downloaded = update_checker::download_update(&app).await?;
            if let Ok(mut pending) = ctx.pending_update.lock() {
                *pending = Some(downloaded);
            }
            let _ = app.emit("update-ready-to-apply", ());
            Ok(())
        }
    }
}

/// Áp dụng bản cập nhật khi user bấm "Khởi động lại".
///
/// - Fallback UAC: có bản đã tải sẵn → chạy installer (UAC), installer đóng + relaunch app.
/// - Đường service: không có bản pending → thoát app để service (SYSTEM) cài đè + relaunch.
#[tauri::command]
pub fn restart_app(ctx: State<'_, AppContextState>) -> Result<(), String> {
    let pending = ctx.pending_update.lock().ok().and_then(|mut p| p.take());

    if let Some((update, bytes)) = pending {
        update
            .install(bytes)
            .map_err(|e| format!("Update install failed: {}", e))?;
        Ok(())
    } else {
        std::process::exit(0);
    }
}

/// Thoát hẳn app — dùng cho nhánh forced update khi user chọn "Thoát" sau khi update thất bại.
#[tauri::command]
pub fn quit_app(app: AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}
