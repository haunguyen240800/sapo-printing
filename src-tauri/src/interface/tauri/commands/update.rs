use tauri::{AppHandle, Emitter};

use crate::infrastructure::platform::updater::update_checker;
use crate::infrastructure::platform::updater::update_checker::InstallGuard;
use crate::application::dto::UpdateCheckResponse;

pub async fn execute_check_for_updates(app: &AppHandle) -> Result<UpdateCheckResponse, String> {
    let result = update_checker::check_for_updates(app).await?;
    Ok(UpdateCheckResponse {
        update_available: result.update_available,
        version: result.version,
        release_notes: result.release_notes,
    })
}

pub async fn execute_install_update(
    app: &AppHandle,
    install_guard: &InstallGuard,
    last_emitted_version: &std::sync::Mutex<Option<String>>,
) -> Result<(), String> {
    let _guard = install_guard
        .try_acquire()
        .ok_or_else(|| "An update is already being installed".to_string())?;

    update_checker::download_and_install_update(app).await?;

    // Reset dedup so periodic check re-notifies if user doesn't restart
    if let Ok(mut version) = last_emitted_version.lock() {
        *version = None;
    }

    // Emit event so frontend can prompt user to restart
    let _ = app.emit("update-ready-to-apply", ());

    Ok(())
}

pub fn execute_restart_app() -> Result<(), String> {
    std::process::exit(0);
}
