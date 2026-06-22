use std::sync::Arc;

use crate::application::errors::Error;
use crate::application::ports::{EventStore, StoredEventData};
use crate::application::services::audit_service::get_audit_trail;

pub struct AuditTrailResult {
    pub events: Vec<StoredEventData>,
}

pub struct GetAuditTrailUseCase {
    event_store: Arc<dyn EventStore>,
}

impl GetAuditTrailUseCase {
    pub fn new(event_store: Arc<dyn EventStore>) -> Self {
        Self { event_store }
    }

    pub fn execute(&self, job_id: &str) -> Result<AuditTrailResult, Error> {
        tracing::info!(
            target = "sapo_printer::application::use_case::get_audit_trail",
            job_id = job_id,
            "GetAuditTrailUseCase: starting"
        );

        let events =
            get_audit_trail(&self.event_store, job_id).map_err(|e| Error::EventStoreError {
                reason: format!("Failed to get audit trail: {}", e),
            })?;

        tracing::debug!(
            target = "sapo_printer::application::use_case::get_audit_trail",
            job_id = job_id,
            total_events = events.len(),
            "GetAuditTrailUseCase: completed"
        );

        Ok(AuditTrailResult { events })
    }
}
