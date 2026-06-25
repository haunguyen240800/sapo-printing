use serde::Serialize;

/// Response DTO for the audit trail of a print job.
#[derive(Clone, Debug, Serialize)]
pub struct AuditTrailResponse {
    pub job_id: String,
    pub events: Vec<AuditEventDto>,
    pub chain_valid: bool,
    pub tampered_count: u64,
}

/// DTO for a single audit event.
#[derive(Clone, Debug, Serialize)]
pub struct AuditEventDto {
    pub sequence_number: i64,
    pub event_type: String,
    pub payload: String,
    pub timestamp: i64,
    pub hmac_valid: bool,
}
