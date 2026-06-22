use serde::Serialize;

#[derive(Serialize)]
pub struct AgentStatusResponse {
    pub port: u16,
    pub paired_count: usize,
}
