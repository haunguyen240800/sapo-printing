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


