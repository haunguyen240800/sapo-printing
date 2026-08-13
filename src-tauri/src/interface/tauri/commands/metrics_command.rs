use tauri::State;

use crate::AppContextState;
use crate::application::models::MetricsResponse;

/// Get operational metrics (job counts, timings).
#[tauri::command]
pub async fn get_metrics(ctx: State<'_, AppContextState>) -> Result<MetricsResponse, String> {
    let use_case = ctx.get_metrics_uc.clone();

    tokio::task::spawn_blocking(move || {
        let snapshot = use_case.execute().map_err(|e| e.to_string())?;
        Ok(MetricsResponse::from(snapshot))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}
