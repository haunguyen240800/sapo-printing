use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::print_job::errors::DomainError;
use crate::infrastructure::database::event_store::{SqliteEventStore, StoredEvent};

type HmacSha256 = Hmac<Sha256>;

/// Report on the integrity of an aggregate's audit trail.
#[derive(Clone, Debug)]
pub struct AuditIntegrityReport {
    pub aggregate_id: String,
    pub total_events: u64,
    pub valid_events: u64,
    pub tampered_events: Vec<i64>,
    pub chain_valid: bool,
}

/// Verify the HMAC integrity of a single stored event.
///
/// Recomputes HMAC from event fields and compares against stored value.
/// Returns `Ok(true)` if valid, `Ok(false)` if tampered.
pub fn verify_event_integrity(
    event: &StoredEvent,
    secret_key: &str,
) -> Result<bool, DomainError> {
    let stored_hmac = match &event.hmac {
        Some(h) => h,
        None => {
            return Err(DomainError::RepositoryError {
                reason: format!(
                    "Event {} has no HMAC (sequence_number={})",
                    event.aggregate_id, event.sequence_number
                ),
            });
        }
    };

    let stored_bytes = hex::decode(stored_hmac).map_err(|e| DomainError::RepositoryError {
        reason: format!("Invalid hex in stored HMAC: {}", e),
    })?;

    let message = format!(
        "{}|{}|{}|{}|{}",
        event.aggregate_id, event.sequence_number, event.event_type, event.payload, event.timestamp
    );
    let key_bytes = hex::decode(secret_key).map_err(|e| DomainError::RepositoryError {
        reason: format!("Invalid hex in signing key: {}", e),
    })?;
    let mut mac = HmacSha256::new_from_slice(&key_bytes)
        .expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    Ok(mac.verify_slice(&stored_bytes).is_ok())
}

/// Retrieve the audit trail for an aggregate, ordered by sequence_number ASC.
///
/// Does NOT verify integrity — caller decides whether to verify.
pub fn get_audit_trail(
    store: &SqliteEventStore,
    aggregate_id: &str,
) -> Result<Vec<StoredEvent>, DomainError> {
    store.find_by_aggregate(aggregate_id)
}

/// Verify integrity of all events in an aggregate's audit trail.
pub fn verify_audit_trail_integrity(
    store: &SqliteEventStore,
    aggregate_id: &str,
    secret_key: &str,
) -> Result<AuditIntegrityReport, DomainError> {
    let events = get_audit_trail(store, aggregate_id)?;
    let total = events.len() as u64;
    let mut valid = 0u64;
    let mut tampered = Vec::new();

    for event in &events {
        if verify_event_integrity(event, secret_key)? {
            valid += 1;
        } else {
            tampered.push(event.sequence_number);
        }
    }

    let chain_valid = tampered.is_empty();

    Ok(AuditIntegrityReport {
        aggregate_id: aggregate_id.to_string(),
        total_events: total,
        valid_events: valid,
        tampered_events: tampered,
        chain_valid,
    })
}

/// Delete events older than `retention_days` from now.
/// Returns count of deleted events.
pub fn cleanup_old_events(
    store: &SqliteEventStore,
    retention_days: u32,
) -> Result<u64, DomainError> {
    if retention_days == 0 {
        return Err(DomainError::RepositoryError {
            reason: "retention_days must be greater than 0".to_string(),
        });
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let cutoff = now - (retention_days as i64 * 86400);
    store.delete_events_before(cutoff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::print_job::aggregate::PrintJob;
    use crate::infrastructure::database::migrations::run_migrations;
    use crate::infrastructure::secrets::SecretManager;
    use crate::shared::errors::InfrastructureError;
    use rusqlite::Connection;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    struct MockSecretManager {
        store: Mutex<HashMap<String, String>>,
    }

    impl MockSecretManager {
        fn new() -> Self {
            Self {
                store: Mutex::new(HashMap::new()),
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

    fn setup_test_store() -> Arc<SqliteEventStore> {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();
        let sm = Arc::new(MockSecretManager::new());
        Arc::new(SqliteEventStore::new(
            Arc::new(Mutex::new(conn)),
            sm.clone(),
        ))
    }

    #[test]
    fn test_verify_event_integrity_valid() {
        let store = setup_test_store();

        let mut job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP".to_string(),
        );
        let job_id = job.id().to_string();
        let events = job.drain_events();
        store
            .save_event(&job_id, events.first().unwrap().as_ref())
            .unwrap();

        let stored = store.find_by_aggregate(&job_id).unwrap();
        assert_eq!(stored.len(), 1);
        assert!(stored[0].hmac.is_some());

        let signing_key = store.get_or_create_signing_key().unwrap();
        let valid = verify_event_integrity(&stored[0], &signing_key).unwrap();
        assert!(valid, "Valid event should pass integrity check");
    }

    #[test]
    fn test_verify_event_integrity_tampered() {
        let store = setup_test_store();

        let mut job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP".to_string(),
        );
        let job_id = job.id().to_string();
        let events = job.drain_events();
        store
            .save_event(&job_id, events.first().unwrap().as_ref())
            .unwrap();

        let mut stored = store.find_by_aggregate(&job_id).unwrap();
        stored[0].payload = "{\"tampered\":true}".to_string();

        let signing_key = store.get_or_create_signing_key().unwrap();
        let valid = verify_event_integrity(&stored[0], &signing_key).unwrap();
        assert!(!valid, "Tampered event should fail integrity check");
    }

    #[test]
    fn test_verify_event_no_hmac() {
        let event = StoredEvent {
            id: 1,
            aggregate_id: "agg-1".to_string(),
            sequence_number: 1,
            event_type: "TestEvent".to_string(),
            payload: "{}".to_string(),
            timestamp: 1700000000,
            hmac: None,
        };
        let result = verify_event_integrity(&event, &"a".repeat(64));
        assert!(result.is_err(), "Event without HMAC should return error");
    }

    #[test]
    fn test_get_audit_trail_ordering() {
        let store = setup_test_store();

        let mut job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP".to_string(),
        );
        let job_id = job.id().to_string();
        job.queue().unwrap();
        job.mark_downloaded().unwrap();
        let events = job.drain_events();
        let count = events.len();

        store.save_all(&job_id, &events).unwrap();

        let trail = get_audit_trail(&store, &job_id).unwrap();
        assert_eq!(trail.len(), count);
        for i in 1..trail.len() {
            assert!(
                trail[i].sequence_number > trail[i - 1].sequence_number,
                "Events should be in ascending sequence order"
            );
        }
    }

    #[test]
    fn test_verify_audit_trail_integrity_all_valid() {
        let store = setup_test_store();

        let mut job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP".to_string(),
        );
        let job_id = job.id().to_string();
        job.queue().unwrap();
        let events = job.drain_events();
        store.save_all(&job_id, &events).unwrap();

        let signing_key = store.get_or_create_signing_key().unwrap();
        let report = verify_audit_trail_integrity(&store, &job_id, &signing_key).unwrap();

        assert_eq!(report.total_events, events.len() as u64);
        assert_eq!(report.valid_events, events.len() as u64);
        assert!(report.tampered_events.is_empty());
        assert!(report.chain_valid);
    }

    #[test]
    fn test_cleanup_preserves_recent_events() {
        let store = setup_test_store();

        let mut job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP".to_string(),
        );
        let job_id = job.id().to_string();
        let events = job.drain_events();
        store
            .save_event(&job_id, events.first().unwrap().as_ref())
            .unwrap();

        let deleted = cleanup_old_events(&store, 30).unwrap();
        assert_eq!(deleted, 0, "Recent events should NOT be deleted");

        let remaining = store.find_by_aggregate(&job_id).unwrap();
        assert_eq!(remaining.len(), 1, "Recent event should still exist");
    }

    #[test]
    fn test_cleanup_deletes_old_events() {
        let store = setup_test_store();

        let mut job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP".to_string(),
        );
        let job_id = job.id().to_string();
        let events = job.drain_events();
        store
            .save_event(&job_id, events.first().unwrap().as_ref())
            .unwrap();

        let future_cutoff = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            + 100_000;

        let deleted = store.delete_events_before(future_cutoff).unwrap();
        assert_eq!(deleted, 1, "Old event should be deleted with future cutoff");

        let remaining = store.find_by_aggregate(&job_id).unwrap();
        assert_eq!(remaining.len(), 0, "No events should remain after cleanup");
    }
}
