use serde::Serialize;

use crate::application::use_cases::get_audit_trail::AuditTrailResult;

#[derive(Clone, Debug, Serialize)]
pub struct AuditTrailResponse {
    pub job_id: String,
    pub events: Vec<AuditEventResponse>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AuditEventResponse {
    pub sequence_number: i64,
    pub event_type: String,
    pub payload: String,
    pub timestamp: i64,
}

impl AuditTrailResponse {
    pub fn from_result(job_id: String, result: AuditTrailResult) -> Self {
        let events: Vec<AuditEventResponse> = result
            .events
            .iter()
            .map(|event| AuditEventResponse {
                sequence_number: event.sequence_number,
                event_type: event.event_type.clone(),
                payload: event.payload.clone(),
                timestamp: event.timestamp,
            })
            .collect();

        Self { job_id, events }
    }
}
