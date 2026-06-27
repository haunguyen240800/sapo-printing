use hmac::{Hmac, Mac};
use rand::RngCore;
use rand::rngs::OsRng;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::models::DomainError;
use crate::domain::common::aggregate::DomainEvent;
use crate::infrastructure::platform::keychain::SecretManager;

type HmacSha256 = Hmac<Sha256>;

/// Compute HMAC-SHA256 over the canonical event string.
///
/// Formula: `HMAC-SHA256(key_bytes, aggregate_id|sequence_number|event_type|payload|timestamp)`
/// Returns lowercase hex string (64 characters).
pub fn compute_hmac(
    secret_key: &str,
    aggregate_id: &str,
    sequence_number: i64,
    event_type: &str,
    payload: &str,
    timestamp: i64,
) -> Result<String, DomainError> {
    let message = format!(
        "{aggregate_id}|{sequence_number}|{event_type}|{payload}|{timestamp}"
    );
    let key_bytes = hex::decode(secret_key).map_err(|e| DomainError::RepositoryError {
        reason: format!("Invalid hex in signing key: {}", e),
    })?;
    let mut mac = HmacSha256::new_from_slice(&key_bytes)
        .expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

/// A domain event as stored in the database.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoredEvent {
    pub id: i64,
    pub aggregate_id: String,
    pub sequence_number: i64,
    pub event_type: String,
    pub payload: String,
    pub timestamp: i64,
    pub hmac: Option<String>,
}

/// SQLite implementation of an Event Store.
///
/// Persists domain events to the `events` table with sequencing per aggregate.
/// Supports batch writes within a transaction for atomicity.
/// Every event is signed with HMAC-SHA256 for tamper detection.
pub struct SqliteEventStore {
    conn: Arc<Mutex<Connection>>,
    secret_manager: Arc<dyn SecretManager>,
    cached_signing_key: Mutex<Option<String>>,
}

impl SqliteEventStore {
    /// Create a new event store with a shared database connection and secret manager.
    pub fn new(conn: Arc<Mutex<Connection>>, secret_manager: Arc<dyn SecretManager>) -> Self {
        Self {
            conn,
            secret_manager,
            cached_signing_key: Mutex::new(None),
        }
    }

    /// Retrieve the HMAC signing key from SecretManager, or generate and store a new one.
    /// The key is cached in memory after first retrieval to prevent TOCTOU races.
    pub fn get_or_create_signing_key(&self) -> Result<String, DomainError> {
        tracing::info!(
            target = "sapo_printer::repository::event_store",
            "get_or_create_signing_key() - checking cache"
        );

        {
            let cache = self.cached_signing_key.lock().unwrap();
            if let Some(ref key) = *cache {
                tracing::info!(
                    target = "sapo_printer::repository::event_store",
                    "get_or_create_signing_key() - found in cache"
                );
                return Ok(key.clone());
            }
        }

        tracing::info!(
            target = "sapo_printer::repository::event_store",
            "get_or_create_signing_key() - calling secret_manager.retrieve()"
        );

        match self.secret_manager.retrieve("hmac_signing_key") {
            Ok(Some(key)) => {
                tracing::info!(
                    target = "sapo_printer::repository::event_store",
                    "get_or_create_signing_key() - retrieved from secret manager"
                );
                let mut cache = self.cached_signing_key.lock().unwrap();
                *cache = Some(key.clone());
                return Ok(key);
            }
            Ok(None) => {
                tracing::info!(
                    target = "sapo_printer::repository::event_store",
                    "get_or_create_signing_key() - key not found, will generate new one"
                );
            }
            Err(e) => {
                tracing::error!(
                    target = "sapo_printer::repository::event_store",
                    error = %e,
                    "get_or_create_signing_key() - failed to retrieve from secret manager"
                );
                return Err(DomainError::RepositoryError {
                    reason: format!("Failed to retrieve signing key: {}", e),
                });
            }
        }

        tracing::info!(
            target = "sapo_printer::repository::event_store",
            "get_or_create_signing_key() - generating new key"
        );

        let mut key_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut key_bytes);
        let key_hex = hex::encode(key_bytes);

        tracing::info!(
            target = "sapo_printer::repository::event_store",
            "get_or_create_signing_key() - storing new key"
        );

        self.secret_manager
            .store("hmac_signing_key", &key_hex)
            .map_err(|e| {
                tracing::error!(
                    target = "sapo_printer::repository::event_store",
                    error = %e,
                    "get_or_create_signing_key() - failed to store new key"
                );
                DomainError::RepositoryError {
                    reason: format!("Failed to store signing key: {}", e),
                }
            })?;

        tracing::info!(
            target = "sapo_printer::repository::event_store",
            "get_or_create_signing_key() - caching new key"
        );

        {
            let mut cache = self.cached_signing_key.lock().unwrap();
            *cache = Some(key_hex.clone());
        }

        tracing::info!(
            target = "sapo_printer::repository::event_store",
            "Generated new HMAC signing key"
        );

        Ok(key_hex)
    }

    /// Persist a single domain event.
    pub fn save_event(
        &self,
        aggregate_id: &str,
        event: &dyn DomainEvent,
    ) -> Result<(), DomainError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());

        let seq = self.next_sequence_number_inner(&conn, aggregate_id)?;
        let payload = event.serialize_payload();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let signing_key = self.get_or_create_signing_key()?;
        let hmac_value = compute_hmac(&signing_key, aggregate_id, seq, event.event_name(), &payload, now)?;

        conn.execute(
            "INSERT INTO events (aggregate_id, sequence_number, event_type, payload, timestamp, hmac)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![aggregate_id, seq, event.event_name(), payload, now, hmac_value],
        )
        .map_err(|e| {
            tracing::error!(
                target = "sapo_printer::repository::event_store",
                operation = "save_event",
                aggregate_id = aggregate_id,
                event_type = event.event_name(),
                error = %e,
                "Failed to save event"
            );
            DomainError::RepositoryError {
                reason: format!("Failed to save event: {}", e),
            }
        })?;

        Ok(())
    }

    /// Persist multiple events in a single transaction (atomic batch).
    /// Locks mutex BEFORE starting the transaction.
    pub fn save_all(
        &self,
        aggregate_id: &str,
        events: &[Box<dyn DomainEvent>],
    ) -> Result<(), DomainError> {
        tracing::info!(
            target = "sapo_printer::repository::event_store",
            operation = "save_all",
            aggregate_id = aggregate_id,
            event_count = events.len(),
            "save_all() STARTING"
        );

        if events.is_empty() {
            return Ok(());
        }

        // IMPORTANT: Get signing key BEFORE locking connection to avoid deadlock
        tracing::info!(
            target = "sapo_printer::repository::event_store",
            "save_all() - calling get_or_create_signing_key()"
        );
        let signing_key = self.get_or_create_signing_key()?;
        tracing::info!(
            target = "sapo_printer::repository::event_store",
            "save_all() - signing key retrieved successfully"
        );

        let mut conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());

        tracing::debug!(
            target = "sapo_printer::repository::event_store",
            operation = "save_all",
            aggregate_id = aggregate_id,
            event_count = events.len(),
            "INSERT INTO events (batch)"
        );

        let base_seq = self.next_sequence_number_inner(&conn, aggregate_id)?;

        let tx = conn
            .transaction()
            .map_err(|e| {
                tracing::error!(
                    target = "sapo_printer::repository::event_store",
                    operation = "save_all",
                    aggregate_id = aggregate_id,
                    error = %e,
                    "Failed to begin transaction"
                );
                DomainError::RepositoryError {
                    reason: format!("Failed to begin transaction: {}", e),
                }
            })?;

        for (i, event) in events.iter().enumerate() {
            let seq = base_seq + i as i64;
            let payload = event.serialize_payload();
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;

            let hmac_value = compute_hmac(&signing_key, aggregate_id, seq, event.event_name(), &payload, now)?;

            if let Err(e) = tx.execute(
                "INSERT INTO events (aggregate_id, sequence_number, event_type, payload, timestamp, hmac)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![aggregate_id, seq, event.event_name(), payload, now, hmac_value],
            ) {
                // Drop tx to trigger automatic rollback (rusqlite rolls back on uncommitted drop).
                // We intentionally do NOT call tx.rollback() here because it takes ownership of self,
                // preventing further use of tx. Dropping achieves the same rollback effect.
                drop(tx);
                return Err(DomainError::RepositoryError {
                    reason: format!("Failed to save event in batch: {}", e),
                });
            }
        }

        tx.commit().map_err(|e| DomainError::RepositoryError {
            reason: format!("Failed to commit transaction: {}", e),
        })?;

        Ok(())
    }

    /// Find all events for a given aggregate, ordered by sequence_number.
    pub fn find_by_aggregate(&self, aggregate_id: &str) -> Result<Vec<StoredEvent>, DomainError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());

        tracing::debug!(
            target = "sapo_printer::repository::event_store",
            operation = "find_by_aggregate",
            aggregate_id = aggregate_id,
            "SELECT FROM events WHERE aggregate_id"
        );

        let mut stmt = conn
            .prepare(
                "SELECT id, aggregate_id, sequence_number, event_type, payload, timestamp, hmac
                 FROM events WHERE aggregate_id = ?1 ORDER BY sequence_number ASC",
            )
            .map_err(|e| {
                tracing::error!(
                    target = "sapo_printer::repository::event_store",
                    operation = "find_by_aggregate",
                    aggregate_id = aggregate_id,
                    error = %e,
                    "Failed to prepare query"
                );
                DomainError::RepositoryError {
                    reason: format!("Failed to prepare query: {}", e),
                }
            })?;

        let event_iter = stmt
            .query_map([aggregate_id], |row| {
                Ok(StoredEvent {
                    id: row.get(0)?,
                    aggregate_id: row.get(1)?,
                    sequence_number: row.get(2)?,
                    event_type: row.get(3)?,
                    payload: row.get(4)?,
                    timestamp: row.get(5)?,
                    hmac: row.get(6)?,
                })
            })
            .map_err(|e| {
                tracing::error!(
                    target = "sapo_printer::repository::event_store",
                    operation = "find_by_aggregate",
                    aggregate_id = aggregate_id,
                    error = %e,
                    "Failed to query events"
                );
                DomainError::RepositoryError {
                    reason: format!("Failed to query events: {}", e),
                }
            })?;

        let mut events = Vec::new();
        for event_result in event_iter {
            events.push(event_result.map_err(|e| DomainError::RepositoryError {
                reason: format!("Failed to read event row: {}", e),
            })?);
        }

        Ok(events)
    }

    /// Get the next sequence number for an aggregate (MAX + 1, starts at 1).
    pub fn next_sequence_number(&self, aggregate_id: &str) -> Result<i64, DomainError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        self.next_sequence_number_inner(&conn, aggregate_id)
    }

    /// Inner: next sequence number given a locked connection reference.
    fn next_sequence_number_inner(
        &self,
        conn: &Connection,
        aggregate_id: &str,
    ) -> Result<i64, DomainError> {
        let result = conn.query_row(
            "SELECT MAX(sequence_number) FROM events WHERE aggregate_id = ?1",
            [aggregate_id],
            |row| row.get::<_, Option<i64>>(0),
        );

        match result {
            Ok(Some(max)) => Ok(max + 1),
            Ok(None) => Ok(1),
            Err(e) => Err(DomainError::RepositoryError {
                reason: format!("Failed to get next sequence number: {}", e),
            }),
        }
    }

    /// Delete events with timestamp older than the given UNIX epoch seconds.
    /// Returns the number of deleted rows.
    pub fn delete_events_before(&self, cutoff_timestamp: i64) -> Result<u64, DomainError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());

        let deleted = conn
            .execute(
                "DELETE FROM events WHERE timestamp < ?1",
                rusqlite::params![cutoff_timestamp],
            )
            .map_err(|e| {
                tracing::error!(
                    target = "sapo_printer::repository::event_store",
                    operation = "delete_events_before",
                    cutoff_timestamp = cutoff_timestamp,
                    error = %e,
                    "Failed to delete old events"
                );
                DomainError::RepositoryError {
                    reason: format!("Failed to delete old events: {}", e),
                }
            })?;

        Ok(deleted as u64)
    }
}


