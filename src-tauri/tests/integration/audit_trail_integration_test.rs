//! Integration tests for HMAC-signed audit trail (Story 4.4).

use rusqlite::Connection;
use std::sync::{Arc, Mutex};

use sapo_printer::domain::print_job::aggregate::PrintJob;
use sapo_printer::infrastructure::database::audit::{
    cleanup_old_events, get_audit_trail, verify_audit_trail_integrity, verify_event_integrity,
};
use sapo_printer::infrastructure::database::migrations::run_migrations;
use sapo_printer::infrastructure::database::SqliteEventStore;
use sapo_printer::infrastructure::secrets::SecretManager;
use sapo_printer::shared::errors::InfrastructureError;

use std::collections::HashMap;

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

fn setup() -> (Arc<SqliteEventStore>, Arc<MockSecretManager>, Arc<Mutex<Connection>>) {
    let mut conn = Connection::open_in_memory().unwrap();
    run_migrations(&mut conn).unwrap();
    let conn = Arc::new(Mutex::new(conn));
    let sm = Arc::new(MockSecretManager::new());
    let store = Arc::new(SqliteEventStore::new(conn.clone(), sm.clone()));
    (store, sm, conn)
}

#[test]
fn test_real_events_have_valid_hmac() {
    let (store, sm, _conn) = setup();

    let mut job = PrintJob::new(
        "https://s3.example.com/doc.pdf".to_string(),
        "HP".to_string(),
    );
    let job_id = job.id().to_string();
    job.queue().unwrap();
    job.mark_downloaded().unwrap();
    let events = job.drain_events();

    store.save_all(&job_id, &events).unwrap();

    let stored = store.find_by_aggregate(&job_id).unwrap();
    assert_eq!(stored.len(), events.len());

    let signing_key = sm.retrieve("hmac_signing_key").unwrap().unwrap();
    for event in &stored {
        assert!(event.hmac.is_some(), "Event should have HMAC");
        assert_eq!(event.hmac.as_ref().unwrap().len(), 64, "HMAC should be 64 hex chars");
        let valid = verify_event_integrity(event, &signing_key).unwrap();
        assert!(valid, "Real event should have valid HMAC");
    }
}

#[test]
fn test_tampered_event_fails_verification() {
    let (store, sm, conn) = setup();

    let mut job = PrintJob::new(
        "https://s3.example.com/doc.pdf".to_string(),
        "HP".to_string(),
    );
    let job_id = job.id().to_string();
    let events = job.drain_events();
    store.save_event(&job_id, events.first().unwrap().as_ref()).unwrap();

    let signing_key = sm.retrieve("hmac_signing_key").unwrap().unwrap();

    let stored = store.find_by_aggregate(&job_id).unwrap();
    assert_eq!(stored.len(), 1);
    let valid = verify_event_integrity(&stored[0], &signing_key).unwrap();
    assert!(valid);

    {
        let c = conn.lock().unwrap();
        c.execute(
            "UPDATE events SET payload = ?1 WHERE aggregate_id = ?2",
            rusqlite::params!["{\"tampered\":true}", job_id],
        )
        .unwrap();
    }

    let tampered = store.find_by_aggregate(&job_id).unwrap();
    assert_eq!(tampered.len(), 1);
    let valid = verify_event_integrity(&tampered[0], &signing_key).unwrap();
    assert!(!valid, "Tampered event should fail HMAC verification");
}

#[test]
fn test_full_lifecycle_audit_trail() {
    let (store, sm, _conn) = setup();

    let mut job = PrintJob::new(
        "https://s3.example.com/doc.pdf".to_string(),
        "HP".to_string(),
    );
    let job_id = job.id().to_string();

    job.queue().unwrap();
    job.mark_downloaded().unwrap();
    job.mark_submitted().unwrap();
    job.mark_printing().unwrap();
    job.complete().unwrap();

    let events = job.drain_events();
    let event_count = events.len();
    store.save_all(&job_id, &events).unwrap();

    let trail = get_audit_trail(&store, &job_id).unwrap();
    assert_eq!(trail.len(), event_count);

    for i in 1..trail.len() {
        assert!(trail[i].sequence_number > trail[i - 1].sequence_number);
    }

    let signing_key = sm.retrieve("hmac_signing_key").unwrap().unwrap();
    let report = verify_audit_trail_integrity(&store, &job_id, &signing_key).unwrap();

    assert_eq!(report.total_events, event_count as u64);
    assert_eq!(report.valid_events, event_count as u64);
    assert!(report.tampered_events.is_empty());
    assert!(report.chain_valid);
}

#[test]
fn test_cleanup_old_events_integration() {
    let (store, _sm, conn) = setup();

    let mut job1 = PrintJob::new(
        "https://s3.example.com/doc1.pdf".to_string(),
        "HP".to_string(),
    );
    let job1_id = job1.id().to_string();
    let events1 = job1.drain_events();
    store.save_event(&job1_id, events1.first().unwrap().as_ref()).unwrap();

    let mut job2 = PrintJob::new(
        "https://s3.example.com/doc2.pdf".to_string(),
        "HP".to_string(),
    );
    let job2_id = job2.id().to_string();
    let events2 = job2.drain_events();
    store.save_event(&job2_id, events2.first().unwrap().as_ref()).unwrap();

    {
        let c = conn.lock().unwrap();
        c.execute(
            "UPDATE events SET timestamp = 1000000 WHERE aggregate_id = ?1",
            rusqlite::params![job1_id],
        )
        .unwrap();
    }

    let deleted = cleanup_old_events(&store, 30).unwrap();
    assert_eq!(deleted, 1, "Only old event should be deleted");

    let remaining1 = store.find_by_aggregate(&job1_id).unwrap();
    assert_eq!(remaining1.len(), 0, "Old event should be gone");

    let remaining2 = store.find_by_aggregate(&job2_id).unwrap();
    assert_eq!(remaining2.len(), 1, "Recent event should be preserved");
}
