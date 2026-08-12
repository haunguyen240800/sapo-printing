use serde::Serialize;

#[derive(Serialize)]
pub struct AgentStatusDto {
    pub port: u16,
    pub paired_count: usize,
}
