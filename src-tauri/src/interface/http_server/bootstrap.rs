//! HTTPS server bootstrap — chuẩn bị state, subscribe SSE, spawn server, watch cert.
//!
//! Gọi từ `main.rs` sau khi core deps ready (DbPool, EventBus).
//!
//! # Trách nhiệm
//! 1. Sinh ApiTokenManager từ DbPool.
//! 2. Sinh SseBroadcaster, subscribe vào EventBus cho các job status events.
//! 3. Load TLS cert từ `data_dir/tls/server.pem` (helper service ghi ra).
//! 4. Bind port fallback, spawn axum-server.
//! 5. Ghi `agent.json` cho webapp discovery.
//! 6. Spawn cert watcher hot reload.

use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::application::services::{ApiTokenManager, UseCaseFactory};
use crate::infrastructure::configs::db::connection::DbPool;
use crate::infrastructure::platform::tls::{
    cert_generator::{CertGenerator, CertPaths},
    cert_installer::{CertInstaller, PlatformInstaller},
    cert_watcher,
};
use crate::shared::errors::InfrastructureError;
use crate::shared::event_bus::EventBus;

use super::server::{self, AgentMetadata};
use super::sse::SseBroadcaster;
use super::state::HttpServerState;

/// Job status events SseBroadcaster subscribe. Chỉ terminal events cho FE.
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
    data_dir: &Path,
    db: DbPool,
    event_bus: Arc<dyn EventBus>,
    use_cases: Arc<UseCaseFactory>,
    app_version: &'static str,
    min_webapp_version: &'static str,
) -> Result<BootstrapResult, InfrastructureError> {
    let paths = CertPaths::under(data_dir);

    // Fallback: nếu helper service chưa sinh cert → app tự sinh.
    // Prod deployment nên chạy helper trước (installer script làm). Dev mode
    // hoặc user không cài helper → app tự sinh + best-effort install CA.
    if !paths.server_pem.exists() || !paths.server_key.exists() {
        tracing::warn!(
            data_dir = %paths.tls_dir().display(),
            "Server cert missing — app tự sinh (helper service chưa chạy)"
        );
        let bundle = CertGenerator::load_or_generate(data_dir)?;
        if bundle.is_newly_generated {
            let installer = PlatformInstaller::default();
            match installer.install_ca(&paths.ca_pem) {
                Ok(_) => tracing::info!("CA installed into system trust store"),
                Err(e) => tracing::warn!(
                    error = %e,
                    "CA install failed (cần admin/root). Browser sẽ hiện cert warning tới khi cài lại app với quyền admin."
                ),
            }
        }
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
