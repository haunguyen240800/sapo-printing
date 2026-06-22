use tauri::State;

use crate::AppContextState;

/// Clear job history (terminal jobs + events), resetting metrics counters.
/// Returns the number of jobs deleted.
#[tauri::command]
pub async fn clear_job_history(ctx: State<'_, AppContextState>) -> Result<u64, String> {
    let use_case = ctx.clear_history_uc.clone();

    tokio::task::spawn_blocking(move || use_case.execute().map_err(|e| e.to_string()))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
}
