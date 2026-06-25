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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::print_job::aggregate::PrintJob;
    use crate::infrastructure::database::migrations::run_migrations;
    use crate::shared::errors::InfrastructureError;
    use rusqlite::Connection;
    use std::collections::HashMap;
    use std::sync::Mutex as StdMutex;

    struct MockSecretManager {
        store: StdMutex<HashMap<String, String>>,
    }

    impl MockSecretManager {
        fn new() -> Self {
            Self {
                store: StdMutex::new(HashMap::new()),
            }
        }
    }

    impl SecretManager for MockSecretManager {
        fn store(&self, key: &str, value: &str) -> Result<(), InfrastructureError> {
            self.store
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_string());
            Ok(())
        }

        fn retrieve(&self, key: &str) -> Result<Option<String>, InfrastructureError> {
            Ok(self.store.lock().unwrap().get(key).cloned())
        }

        fn delete(&self, key: &str) -> Result<(), InfrastructureError> {
            self.store.lock().unwrap().remove(key);
            Ok(())
        }
    }

    fn setup() -> (Arc<SqliteEventStore>, Arc<MockSecretManager>) {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();
        let sm = Arc::new(MockSecretManager::new());
        let store = Arc::new(SqliteEventStore::new(
            Arc::new(StdMutex::new(conn)),
            sm.clone(),
        ));
        (store, sm)
    }

    #[test]
    fn test_execute_returns_events_and_report() {
        let (store, sm) = setup();

        let mut job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP".to_string(),
        );
        let job_id = job.id().to_string();
        job.queue().unwrap();
        let events = job.drain_events();
        store.save_all(&job_id, &events).unwrap();

        let use_case = AuditTrailUseCase::new(store, sm);
        let result = use_case.execute(&job_id).unwrap();

        assert_eq!(result.events.len(), events.len());
        assert!(result.report.chain_valid);
        assert_eq!(result.report.total_events, events.len() as u64);
        assert_eq!(result.event_hmac_valid.len(), events.len());
        assert!(result.event_hmac_valid.iter().all(|&v| v));
    }

    #[test]
    fn test_execute_empty_trail() {
        let (store, sm) = setup();

        let use_case = AuditTrailUseCase::new(store, sm);
        let result = use_case
            .execute("00000000-0000-0000-0000-000000000000")
            .unwrap();

        assert_eq!(result.events.len(), 0);
        assert_eq!(result.report.total_events, 0);
        assert!(result.report.chain_valid);
    }

    #[test]
    fn test_execute_missing_signing_key() {
        let (store, _sm) = setup();

        let mut job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP".to_string(),
        );
        let job_id = job.id().to_string();
        let events = job.drain_events();
        store.save_all(&job_id, &events).unwrap();

        let empty_sm = Arc::new(MockSecretManager::new());
        let use_case = AuditTrailUseCase::new(store, empty_sm);
        let result = use_case.execute(&job_id);

        assert!(result.is_err());
    }
}
