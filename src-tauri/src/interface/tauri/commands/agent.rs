//! Tauri commands cho quản lý HTTPS agent + pairing + cert renew.

use std::sync::Arc;

use serde::Serialize;
use tauri::State;
use uuid::Uuid;

use crate::application::services::{ApiTokenManager, PairedOrigin};
use crate::infrastructure::platform::tls::{ipc::IpcRequest, ipc_client};

/// State object cho HTTPS agent. Register vào Tauri qua `.manage()`.
pub struct AgentState {
    pub token_manager: Arc<ApiTokenManager>,
    pub agent_port: u16,
}

#[derive(Serialize)]
pub struct AgentStatusDto {
    pub port: u16,
    pub paired_count: usize,
}

#[tauri::command]
pub fn get_agent_port(state: State<'_, AgentState>) -> u16 {
    state.agent_port
}

#[tauri::command]
pub fn get_paired_origins(
    state: State<'_, AgentState>,
) -> Result<Vec<PairedOrigin>, String> {
    state.token_manager.list_paired().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn revoke_token(state: State<'_, AgentState>, token_hash: String) -> Result<(), String> {
    state
        .token_manager
        .revoke(&token_hash)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn resolve_pair(
    state: State<'_, AgentState>,
    request_id: String,
    approved: bool,
) -> Result<bool, String> {
    let id = Uuid::parse_str(&request_id).map_err(|e| format!("bad request_id: {}", e))?;
    Ok(state.token_manager.resolve_pair(id, approved).await)
}

#[tauri::command]
pub async fn renew_cert_now() -> Result<serde_json::Value, String> {
    let resp = ipc_client::send(IpcRequest::RenewNow)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(resp).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_agent_status(state: State<'_, AgentState>) -> Result<AgentStatusDto, String> {
    let count = state
        .token_manager
        .list_paired()
        .map(|v| v.len())
        .unwrap_or(0);
    Ok(AgentStatusDto {
        port: state.agent_port,
        paired_count: count,
    })
}
