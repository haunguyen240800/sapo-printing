use std::sync::Arc;

use tauri::State;
use uuid::Uuid;

use crate::application::ports::ApiTokenPort;

pub struct AgentState {
    pub token_manager: Arc<dyn ApiTokenPort>,
    pub agent_port: u16,
}

#[tauri::command]
pub async fn approve_pairing_request(
    state: State<'_, AgentState>,
    request_id: String,
    approved: bool,
) -> Result<bool, String> {
    let id = Uuid::parse_str(&request_id).map_err(|e| format!("bad request_id: {}", e))?;
    Ok(state.token_manager.resolve_pair(id, approved).await)
}
