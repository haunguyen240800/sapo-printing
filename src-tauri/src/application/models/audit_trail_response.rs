use serde::Serialize;

use crate::application::use_cases::get_audit_trail::AuditTrailResult;

#[derive(Clone, Debug, Serialize)]
pub struct AuditTrailResponse {
    pub job_id: String,
    pub events: Vec<AuditEventResponse>,
    pub chain_valid: bool,
    pub tampered_count: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct AuditEventResponse {
    pub sequence_number: i64,
    pub event_type: String,
    pub payload: String,
    pub timestamp: i64,
    pub hmac_valid: bool,
}

impl AuditTrailResponse {
    pub fn from_result(job_id: String, result: AuditTrailResult) -> Self {
        let events: Vec<AuditEventResponse> = result
            .events
            .iter()
            .enumerate()
            .map(|(i, event)| AuditEventResponse {
                sequence_number: event.sequence_number,
                event_type: event.event_type.clone(),
                payload: event.payload.clone(),
                timestamp: event.timestamp,
                hmac_valid: result.event_hmac_valid.get(i).copied().unwrap_or(false),
            })
            .collect();

        Self {
            job_id,
            events,
            chain_valid: result.report.chain_valid,
            tampered_count: result.report.tampered_events.len() as u64,
        }
    }
}
