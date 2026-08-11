use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::application::services::{ApiTokenManager, UseCaseFactory};
use crate::infrastructure::configs::db::connection::DbPool;
use crate::infrastructure::platform::tls::{
    cert_generator::CertPaths,
    cert_watcher,
};
use crate::shared::errors::InfrastructureError;
use crate::shared::event_bus::EventBus;

use super::server::{self, AgentMetadata};
use super::sse::SseBroadcaster;
use super::state::HttpServerState;

pub const JOB_STATUS_EVENTS: &[&str] = &[
    "PrintJobCompleted",
    "PrintJobFailed",
];

pub struct BootstrapResult {
    pub token_manager: Arc<ApiTokenManager>,
    pub sse_broadcaster: Arc<SseBroadcaster>,
    pub port: u16,
}

pub async fn start(
    cert_dir: &Path,
    data_dir: &Path,
    db: DbPool,
    event_bus: Arc<dyn EventBus>,
    use_cases: Arc<UseCaseFactory>,
    app_version: &'static str,
    min_webapp_version: &'static str,
) -> Result<BootstrapResult, InfrastructureError> {
    // Cert (bao gồm CA) CHỈ được provision bởi cert-manager (elevated) lúc CÀI ĐẶT app.
    // App runtime CHỈ đọc cert ở đây — KHÔNG tự sinh CA/cert. User process không có
    // quyền cài CA vào trust store, và CA tự sinh sẽ không được trust => web call sẽ lỗi.
    let paths = CertPaths::under(cert_dir);

    if !paths.server_pem.exists() || !paths.server_key.exists() {
        tracing::error!(
            cert_dir = %paths.tls_dir().display(),
            "Server cert missing — cert-manager chưa provision. Cần cài lại app với \
             quyền admin để cert-manager sinh + trust CA lúc cài đặt."
        );
        return Err(InfrastructureError::SecretServiceUnavailable(format!(
            "TLS cert chưa được provision tại {}. Hãy cài lại app với quyền admin \
             để cert-manager sinh cert + trust CA.",
            paths.tls_dir().display()
        )));
    }

    let token_manager = ApiTokenManager::new(db);

    let broadcaster = SseBroadcaster::new();
    for event_type in JOB_STATUS_EVENTS {
        event_bus.subscribe(event_type, broadcaster.clone() as Arc<dyn crate::shared::event_bus::EventHandler>);
    }

    let broadcaster_for_state = broadcaster.clone();
    let tm_for_state = token_manager.clone();

    let handles = server::start_server(
        paths.server_pem.clone(),
        paths.server_key.clone(),
        move |port| HttpServerState {
            token_manager: tm_for_state,
            app_version,
            min_webapp_version,
            agent_port: port,
            sse_broadcaster: Some(broadcaster_for_state),
            use_cases,
        },
    )
        .await?;

    AgentMetadata {
        port: handles.port,
        version: app_version.into(),
        started_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
    }
        .write(data_dir)?;

    cert_watcher::spawn_watcher(
        paths.server_pem.clone(),
        paths.server_key.clone(),
        handles.tls_config.clone(),
    )?;

    Ok(BootstrapResult {
        token_manager,
        sse_broadcaster: broadcaster,
        port: handles.port,
    })
}
