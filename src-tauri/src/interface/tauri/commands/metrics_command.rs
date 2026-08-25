use tauri::State;

use crate::AppContextState;
use crate::application::ports::MetricsSnapshot;

/// Get operational metrics (job counts, timings).
#[tauri::command]
pub async fn get_metrics(ctx: State<'_, AppContextState>) -> Result<MetricsSnapshot, String> {
    let use_case = ctx.get_metrics_uc.clone();

    tokio::task::spawn_blocking(move || use_case.execute().map_err(|e| e.to_string()))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
}
