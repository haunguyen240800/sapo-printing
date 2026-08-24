use tauri::{AppHandle, Emitter, State};

use crate::AppContextState;
use crate::application::models::UpdateCheckResponse;
use crate::infrastructure::platform::updater::update_checker;

#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> Result<UpdateCheckResponse, String> {
    let result = update_checker::check_for_updates(&app).await?;
    Ok(UpdateCheckResponse {
        update_available: result.update_available,
        version: result.version,
        release_notes: result.release_notes,
    })
}

#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    ctx: State<'_, AppContextState>,
) -> Result<String, String> {
    let _guard = ctx
        .install_guard
        .try_acquire()
        .ok_or_else(|| "An update is already being installed".to_string())?;

    if let Ok(mut version) = ctx.last_emitted_update_version.lock() {
        *version = None;
    }

    let downloaded = update_checker::download_update(&app).await?;
    let target_version = downloaded.0.version.clone();
    let mut pending = ctx
        .pending_update
        .lock()
        .map_err(|_| "Update state is unavailable".to_string())?;
    *pending = Some(downloaded);
    drop(pending);

    tracing::info!(
        target = "sapo_printer::updater",
        version = %target_version,
        "Update downloaded; waiting for user to open the installer"
    );
    let _ = app.emit("update-ready-to-apply", target_version.clone());
    Ok(target_version)
}

#[tauri::command]
pub fn restart_app(ctx: State<'_, AppContextState>) -> Result<(), String> {
    let _guard = ctx
        .install_guard
        .try_acquire()
        .ok_or_else(|| "An update is already being applied".to_string())?;
    let pending = ctx
        .pending_update
        .lock()
        .map_err(|_| "Update state is unavailable".to_string())?;
    let (update, bytes) = pending
        .as_ref()
        .ok_or_else(|| "No downloaded update is ready".to_string())?;
    update
        .install(bytes)
        .map_err(|e| format!("Update install failed: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn quit_app(app: AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}
