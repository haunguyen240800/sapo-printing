use std::sync::Arc;

use crate::application::errors::ApplicationError;
use crate::application::ports::{EventStore, SecretManager, StoredEventData};
use crate::application::services::audit_service::{
    get_audit_trail, verify_audit_trail_integrity, verify_event_integrity, AuditIntegrityReport,
};

pub struct AuditTrailResult {
    pub events: Vec<StoredEventData>,
    pub report: AuditIntegrityReport,
    pub event_hmac_valid: Vec<bool>,
}

pub struct GetAuditTrailUseCase {
    event_store: Arc<dyn EventStore>,
    secret_manager: Arc<dyn SecretManager>,
}

impl GetAuditTrailUseCase {
    pub fn new(
        event_store: Arc<dyn EventStore>,
        secret_manager: Arc<dyn SecretManager>,
    ) -> Self {
        Self {
            event_store,
            secret_manager,
        }
    }

    pub fn execute(&self, job_id: &str) -> Result<AuditTrailResult, ApplicationError> {
        tracing::info!(
            target = "sapo_printer::application::use_case::get_audit_trail",
            job_id = job_id,
            "GetAuditTrailUseCase: starting"
        );

        let events = get_audit_trail(&self.event_store, job_id).map_err(|e| {
            ApplicationError::EventStoreError {
                reason: format!("Failed to get audit trail: {}", e),
            }
        })?;

        if events.is_empty() {
            let report = AuditIntegrityReport {
                aggregate_id: job_id.to_string(),
                total_events: 0,
                valid_events: 0,
                tampered_events: Vec::new(),
                chain_valid: true,
            };
            return Ok(AuditTrailResult {
                events,
                report,
                event_hmac_valid: Vec::new(),
            });
        }

        let signing_key = self
            .secret_manager
            .retrieve("hmac_signing_key")
            .map_err(|e| {
                ApplicationError::RepositoryError(format!(
                    "Failed to retrieve signing key: {}",
                    e
                ))
            })?
            .ok_or_else(|| {
                ApplicationError::RepositoryError(
                    "HMAC signing key not found in secret store".to_string(),
                )
            })?;

        let report =
            verify_audit_trail_integrity(&self.event_store, job_id, &signing_key).map_err(
                |e| ApplicationError::EventStoreError {
                    reason: format!("Failed to verify audit trail: {}", e),
                },
            )?;

        let event_hmac_valid: Vec<bool> = events
            .iter()
            .map(|event| {
                verify_event_integrity(event, &signing_key)
                    .unwrap_or(false)
            })
            .collect();

        tracing::debug!(
            target = "sapo_printer::application::use_case::get_audit_trail",
            job_id = job_id,
            total_events = report.total_events,
            chain_valid = report.chain_valid,
            "GetAuditTrailUseCase: completed"
        );

        Ok(AuditTrailResult {
            events,
            report,
            event_hmac_valid,
        })
    }
}
