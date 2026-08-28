use std::sync::Arc;

use tauri::{App, Emitter, Manager};

use crate::{
    AppContextState,
    infrastructure::{configs::db::DbPool, persistence::ApiTokenRepository},
    interface::http_server,
    interface::tauri::commands::auth_command::AgentState,
};

/// Khởi động HTTP loopback agent server trong background task.
/// Emit `agent-pair-request` về frontend khi có pairing request mới.
pub fn start_http_server(app: &App, pool: DbPool, data_dir: &std::path::Path) {
    let data_dir = data_dir.to_path_buf();

    let token_manager: Arc<dyn crate::application::ports::ApiTokenPort> =
        ApiTokenRepository::new(pool);

    let ctx = app.state::<AppContextState>();
    // event_bus được lấy từ CreatePrintJobUseCase — cùng Arc nên share đúng instance
    let event_bus = ctx.create_print_job_uc.event_bus.clone();
    let create_uc = ctx.create_print_job_uc.clone();
    let get_status_uc = ctx.get_job_status_uc.clone();
    let cancel_print_job_uc = ctx.cancel_print_job_uc.clone();
    let app_handle = app.handle().clone();

    tauri::async_runtime::spawn(async move {
        match http_server::start_bootstrap(
            &data_dir,
            token_manager,
            event_bus,
            create_uc,
            get_status_uc,
            cancel_print_job_uc,
            env!("SAPO_APP_VERSION"),
        )
        .await
        {
            Ok(result) => {
                tracing::info!(port = result.port, "HTTP loopback agent started");

                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                result.token_manager.set_ui_sink(tx).await;

                let emit_handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    while let Some(req) = rx.recv().await {
                        let payload = serde_json::json!({
                            "request_id": req.request_id.to_string(),
                            "origin": req.origin,
                        });
                        let _ = emit_handle.emit("agent-pair-request", payload);
                    }
                });

                app_handle.manage(AgentState {
                    token_manager: result.token_manager,
                    agent_port: result.port,
                });
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    "HTTP agent bootstrap failed — webapp integration disabled"
                );
            }
        }
    });
}
