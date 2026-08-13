use tauri::{AppHandle, Emitter, State};

use crate::AppContextState;
use crate::application::models::UpdateCheckResponse;
use crate::infrastructure::platform::updater::update_checker;

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
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    ctx: State<'_, AppContextState>,
) -> Result<(), String> {
    let _guard = ctx
        .install_guard
        .try_acquire()
        .ok_or_else(|| "An update is already being installed".to_string())?;

    update_checker::download_and_install_update(&app).await?;

    // Reset dedup so periodic check re-notifies if user doesn't restart
    if let Ok(mut version) = ctx.last_emitted_update_version.lock() {
        *version = None;
    }

    // Emit event so frontend can prompt user to restart
    let _ = app.emit("update-ready-to-apply", ());

    Ok(())
}

/// Restart the application (used after an update is installed).
#[tauri::command]
pub fn restart_app() -> Result<(), String> {
    std::process::exit(0);
}
