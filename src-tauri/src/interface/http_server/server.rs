//! HTTPS server bootstrap — bind + load TLS cert + spawn axum-server.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum_server::tls_rustls::RustlsConfig;
use serde::{Deserialize, Serialize};

use crate::infrastructure::platform::tls::port_binder::{self, DEFAULT_PORT, FALLBACK_RANGE};
use crate::shared::errors::InfrastructureError;

use super::router;
use super::state::HttpServerState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetadata {
    pub port: u16,
    pub version: String,
    pub started_at: i64,
}

impl AgentMetadata {
    pub fn write(&self, data_dir: &Path) -> Result<(), InfrastructureError> {
        let path = data_dir.join("agent.json");
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| InfrastructureError::TlsError(format!("serialize agent.json: {}", e)))?;
        std::fs::write(&path, json)?;
        Ok(())
    }

    pub fn read(data_dir: &Path) -> Result<Self, InfrastructureError> {
        let path = data_dir.join("agent.json");
        let s = std::fs::read_to_string(&path)?;
        serde_json::from_str(&s)
            .map_err(|e| InfrastructureError::TlsError(format!("parse agent.json: {}", e)))
    }
}

pub struct ServerHandles {
    pub port: u16,
    pub tls_config: Arc<RustlsConfig>,
}

/// Bind port + load TLS + start axum-server. Returns immediately after spawn.
pub async fn start_server(
    server_pem: PathBuf,
    server_key: PathBuf,
    state_builder: impl FnOnce(u16) -> HttpServerState,
) -> Result<ServerHandles, InfrastructureError> {
    let (std_listener, port) = port_binder::bind_with_fallback(DEFAULT_PORT, FALLBACK_RANGE)
        .map_err(|e| InfrastructureError::TlsError(format!("bind port: {}", e)))?;
    std_listener
        .set_nonblocking(true)
        .map_err(|e| InfrastructureError::TlsError(format!("set_nonblocking: {}", e)))?;

    let tls = load_rustls_config(&server_pem, &server_key).await?;
    let state = state_builder(port);
    let app = router::build(state);

    let listener = tokio::net::TcpListener::from_std(std_listener)
        .map_err(|e| InfrastructureError::TlsError(format!("from_std listener: {}", e)))?
        .into_std()
        .map_err(|e| InfrastructureError::TlsError(format!("into_std listener: {}", e)))?;

    let tls_arc = Arc::new(tls);
    let tls_for_server = tls_arc.clone();
    tokio::spawn(async move {
        let server = axum_server::from_tcp_rustls(listener, (*tls_for_server).clone())
            .serve(app.into_make_service());
        if let Err(e) = server.await {
            tracing::error!(error = %e, "HTTPS server exited");
        }
    });

    Ok(ServerHandles {
        port,
        tls_config: tls_arc,
    })
}

async fn load_rustls_config(
    cert: &Path,
    key: &Path,
) -> Result<RustlsConfig, InfrastructureError> {
    RustlsConfig::from_pem_file(cert, key)
        .await
        .map_err(|e| InfrastructureError::TlsError(format!("load TLS pem: {}", e)))
}
