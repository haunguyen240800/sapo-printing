use std::sync::Arc;

use crate::application::use_cases::errors::ApplicationError;
use crate::infrastructure::database::audit::{
    get_audit_trail, verify_audit_trail_integrity, AuditIntegrityReport,
};
use crate::infrastructure::database::event_store::StoredEvent;
use crate::infrastructure::database::SqliteEventStore;
use crate::infrastructure::secrets::SecretManager;

/// Result of an audit trail retrieval.
pub struct AuditTrailResult {
    pub events: Vec<StoredEvent>,
    pub report: AuditIntegrityReport,
    /// Per-event HMAC validity, aligned with `events` by index.
    pub event_hmac_valid: Vec<bool>,
}

/// Use case: Retrieve and verify the audit trail for a print job.
pub struct AuditTrailUseCase {
    event_store: Arc<SqliteEventStore>,
    secret_manager: Arc<dyn SecretManager>,
}

impl AuditTrailUseCase {
    pub fn new(
        event_store: Arc<SqliteEventStore>,
        secret_manager: Arc<dyn SecretManager>,
    ) -> Self {
        Self {
            event_store,
            secret_manager,
        }
    }

    /// Execute the use case: retrieve audit trail for a job and verify integrity.
    pub fn execute(&self, job_id: &str) -> Result<AuditTrailResult, ApplicationError> {
        tracing::info!(
            target = "sapo_printer::use_case::get_audit_trail",
            job_id = job_id,
            "AuditTrailUseCase: starting"
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
                crate::infrastructure::database::audit::verify_event_integrity(event, &signing_key)
                    .unwrap_or(false)
            })
            .collect();

        tracing::debug!(
            target = "sapo_printer::use_case::get_audit_trail",
            job_id = job_id,
            total_events = report.total_events,
            chain_valid = report.chain_valid,
            "AuditTrailUseCase: completed"
        );

        Ok(AuditTrailResult {
            events,
            report,
            event_hmac_valid,
        })
    }
}


