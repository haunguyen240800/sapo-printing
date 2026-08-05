use hmac::{Hmac, Mac};
use rand::rngs::OsRng;
use rand::RngCore;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::application::ports::{EventStore, SecretManager};
use crate::domain::common::aggregate::DomainEvent;
use crate::domain::print_job::PrintJobError;
use crate::infrastructure::configs::db::{DbPool, SqliteConn};

type HmacSha256 = Hmac<Sha256>;

pub fn compute_hmac(
    secret_key: &str,
    aggregate_id: &str,
    sequence_number: i64,
    event_type: &str,
    payload: &str,
    timestamp: i64,
) -> Result<String, PrintJobError> {
    let message = format!(
        "{aggregate_id}|{sequence_number}|{event_type}|{payload}|{timestamp}"
    );
    let key_bytes = hex::decode(secret_key).map_err(|e| PrintJobError::RepositoryError {
        reason: format!("Invalid hex in signing key: {}", e),
    })?;
    let mut mac = HmacSha256::new_from_slice(&key_bytes)
        .expect("HMAC can take key of any size");
    mac.update(message.as_bytes());
    Ok(hex::encode(mac.finalize().into_bytes()))
}

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

pub struct SqliteEventStore {
    pool: DbPool,
    secret_manager: Arc<dyn SecretManager>,
    cached_signing_key: Mutex<Option<String>>,
}

impl SqliteEventStore {
    pub fn new(pool: DbPool, secret_manager: Arc<dyn SecretManager>) -> Self {
        Self {
            pool,
            secret_manager,
            cached_signing_key: Mutex::new(None),
        }
    }

    fn acquire(&self) -> Result<SqliteConn, PrintJobError> {
        self.pool.get().map_err(|e| PrintJobError::RepositoryError {
            reason: format!("Failed to acquire DB connection: {}", e),
        })
    }

    pub fn get_or_create_signing_key(&self) -> Result<String, PrintJobError> {
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
                return Err(PrintJobError::RepositoryError {
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
                PrintJobError::RepositoryError {
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

    pub fn save_event(
        &self,
        aggregate_id: &str,
        event: &dyn DomainEvent,
    ) -> Result<(), PrintJobError> {
        let signing_key = self.get_or_create_signing_key()?;

        let mut conn = self.acquire()?;
        let tx = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| PrintJobError::RepositoryError {
                reason: format!("Failed to begin transaction: {}", e),
            })?;

        let seq = self.next_sequence_number_inner(&*tx, aggregate_id)?;
        let payload = event.serialize_payload();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let hmac_value = compute_hmac(&signing_key, aggregate_id, seq, event.event_name(), &payload, now)?;

        tx.execute(
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
                PrintJobError::RepositoryError {
                    reason: format!("Failed to save event: {}", e),
                }
            })?;

        tx.commit().map_err(|e| PrintJobError::RepositoryError {
            reason: format!("Failed to commit save_event transaction: {}", e),
        })?;

        Ok(())
    }

    pub fn save_all(
        &self,
        aggregate_id: &str,
        events: &[Box<dyn DomainEvent>],
    ) -> Result<(), PrintJobError> {
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

        let mut conn = self.acquire()?;

        tracing::debug!(
            target = "sapo_printer::repository::event_store",
            operation = "save_all",
            aggregate_id = aggregate_id,
            event_count = events.len(),
            "INSERT INTO events (batch)"
        );

        let base_seq = self.next_sequence_number_inner(&*conn, aggregate_id)?;

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
                PrintJobError::RepositoryError {
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
                return Err(PrintJobError::RepositoryError {
                    reason: format!("Failed to save event in batch: {}", e),
                });
            }
        }

        tx.commit().map_err(|e| PrintJobError::RepositoryError {
            reason: format!("Failed to commit transaction: {}", e),
        })?;

        Ok(())
    }

    pub fn find_by_aggregate(&self, aggregate_id: &str) -> Result<Vec<StoredEvent>, PrintJobError> {
        let conn = self.acquire()?;

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
                PrintJobError::RepositoryError {
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
                PrintJobError::RepositoryError {
                    reason: format!("Failed to query events: {}", e),
                }
            })?;

        let mut events = Vec::new();
        for event_result in event_iter {
            events.push(event_result.map_err(|e| PrintJobError::RepositoryError {
                reason: format!("Failed to read event row: {}", e),
            })?);
        }

        Ok(events)
    }

    pub fn next_sequence_number(&self, aggregate_id: &str) -> Result<i64, PrintJobError> {
        let conn = self.acquire()?;
        self.next_sequence_number_inner(&*conn, aggregate_id)
    }

    fn next_sequence_number_inner(
        &self,
        conn: &Connection,
        aggregate_id: &str,
    ) -> Result<i64, PrintJobError> {
        let result = conn.query_row(
            "SELECT MAX(sequence_number) FROM events WHERE aggregate_id = ?1",
            [aggregate_id],
            |row| row.get::<_, Option<i64>>(0),
        );

        match result {
            Ok(Some(max)) => Ok(max + 1),
            Ok(None) => Ok(1),
            Err(e) => Err(PrintJobError::RepositoryError {
                reason: format!("Failed to get next sequence number: {}", e),
            }),
        }
    }

    pub fn delete_events_before(&self, cutoff_timestamp: i64) -> Result<u64, PrintJobError> {
        let conn = self.acquire()?;

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
                PrintJobError::RepositoryError {
                    reason: format!("Failed to delete old events: {}", e),
                }
            })?;

        Ok(deleted as u64)
    }
}

impl EventStore for SqliteEventStore {
    fn save_all(
        &self,
        aggregate_id: &str,
        events: &[Box<dyn DomainEvent>],
    ) -> Result<(), PrintJobError> {
        self.save_all(aggregate_id, events)
    }

    fn find_by_aggregate(
        &self,
        aggregate_id: &str,
    ) -> Result<Vec<crate::application::ports::StoredEventData>, PrintJobError> {
        self.find_by_aggregate(aggregate_id).map(|events| {
            events
                .into_iter()
                .map(|e| crate::application::ports::StoredEventData {
                    id: e.id,
                    aggregate_id: e.aggregate_id,
                    sequence_number: e.sequence_number,
                    event_type: e.event_type,
                    payload: e.payload,
                    timestamp: e.timestamp,
                    hmac: e.hmac,
                })
                .collect()
        })
    }

    fn delete_events_before(&self, cutoff_timestamp: i64) -> Result<u64, PrintJobError> {
        self.delete_events_before(cutoff_timestamp)
    }
}
