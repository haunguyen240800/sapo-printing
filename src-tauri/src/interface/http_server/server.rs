use std::path::Path;

use axum::Router;
use serde::{Deserialize, Serialize};

use crate::infrastructure::errors::InfrastructureError;
use crate::infrastructure::platform::port_binder::{self, DEFAULT_PORT};

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
        let json = serde_json::to_string_pretty(self).map_err(|e| {
            InfrastructureError::SerializationError(format!("serialize agent.json: {e}"))
        })?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn read(data_dir: &Path) -> Result<Self, InfrastructureError> {
        let path = data_dir.join("agent.json");
        let contents = std::fs::read_to_string(path)?;
        serde_json::from_str(&contents)
            .map_err(|e| InfrastructureError::SerializationError(format!("parse agent.json: {e}")))
    }
}

pub struct ServerHandles {
    pub port: u16,
}

pub async fn start_server(
    state_builder: impl FnOnce(u16) -> HttpServerState,
) -> Result<ServerHandles, InfrastructureError> {
    let std_listener = port_binder::bind()
        .map_err(|e| InfrastructureError::BindError(format!("bind port {DEFAULT_PORT}: {e}")))?;
    let port = DEFAULT_PORT;
    std_listener
        .set_nonblocking(true)
        .map_err(|e| InfrastructureError::IoError(format!("set_nonblocking: {e}")))?;

    let app = router::build(state_builder(port));
    let listener = tokio::net::TcpListener::from_std(std_listener)
        .map_err(|e| InfrastructureError::IoError(format!("from_std listener: {e}")))?;

    spawn_server(listener, app);

    Ok(ServerHandles { port })
}

fn spawn_server(listener: tokio::net::TcpListener, app: Router) {
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app.into_make_service()).await {
            tracing::error!(error = %e, "HTTP server exited");
        }
    });
}

#[cfg(test)]
mod tests {
    use axum::{http::StatusCode, routing::get};

    use super::*;

    #[tokio::test]
    async fn serves_ping_over_plain_http_without_certificates() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new().route(
            "/api/v1/ping",
            get(|| async { (StatusCode::OK, r#"{"status":"ok"}"#) }),
        );

        spawn_server(listener, app);

        let response = reqwest::get(format!("http://{address}/api/v1/ping"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.text().await.unwrap(), r#"{"status":"ok"}"#);
    }
}
