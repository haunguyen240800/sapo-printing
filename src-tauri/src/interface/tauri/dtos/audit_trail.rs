use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct AuditTrailResponse {
    pub job_id: String,
    pub events: Vec<AuditEventDto>,
    pub chain_valid: bool,
    pub tampered_count: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct AuditEventDto {
    pub sequence_number: i64,
    pub event_type: String,
    pub payload: String,
    pub timestamp: i64,
    pub hmac_valid: bool,
}
