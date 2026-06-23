use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::print_job::errors::DomainError;
use crate::domain::print_job::events::DomainEvent;

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
pub struct SqliteEventStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteEventStore {
    /// Create a new event store with a shared database connection.
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
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

        conn.execute(
            "INSERT INTO events (aggregate_id, sequence_number, event_type, payload, timestamp, hmac)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![aggregate_id, seq, event.event_type(), payload, now, None::<String>],
        )
        .map_err(|e| DomainError::RepositoryError {
            reason: format!("Failed to save event: {}", e),
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
        let mut conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());

        let base_seq = self.next_sequence_number_inner(&conn, aggregate_id)?;
        let tx = conn
            .transaction()
            .map_err(|e| DomainError::RepositoryError {
                reason: format!("Failed to begin transaction: {}", e),
            })?;

        for (i, event) in events.iter().enumerate() {
            let seq = base_seq + i as i64;
            let payload = event.serialize_payload();
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;

            if let Err(e) = tx.execute(
                "INSERT INTO events (aggregate_id, sequence_number, event_type, payload, timestamp, hmac)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![aggregate_id, seq, event.event_type(), payload, now, None::<String>],
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

        let mut stmt = conn
            .prepare(
                "SELECT id, aggregate_id, sequence_number, event_type, payload, timestamp, hmac
                 FROM events WHERE aggregate_id = ?1 ORDER BY sequence_number ASC",
            )
            .map_err(|e| DomainError::RepositoryError {
                reason: format!("Failed to prepare query: {}", e),
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
            .map_err(|e| DomainError::RepositoryError {
                reason: format!("Failed to query events: {}", e),
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::print_job::aggregate::PrintJob;
    use crate::infrastructure::database::migrations::run_migrations;

    fn setup_test_db() -> Arc<Mutex<Connection>> {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();
        Arc::new(Mutex::new(conn))
    }

    #[test]
    fn test_save_single_event() {
        let conn = setup_test_db();
        let store = SqliteEventStore::new(conn);

        let job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP".to_string(),
        );
        let job_id = job.id().to_string();
        let events = job.drain_events();

        store
            .save_event(&job_id, events.first().unwrap().as_ref())
            .unwrap();

        let found = store.find_by_aggregate(&job_id).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].event_type, "PrintJobCreated");
        assert_eq!(found[0].sequence_number, 1);
    }

    #[test]
    fn test_save_all_batch() {
        let conn = setup_test_db();
        let store = SqliteEventStore::new(conn);

        // Create a job and transition through states to accumulate events
        let mut job = PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP".to_string(),
        );
        let job_id = job.id().to_string();
        job.queue().unwrap();
        job.mark_downloaded().unwrap();

        let events = job.drain_events();
        // Events: PrintJobCreated (seq 1), PrintJobQueued (seq 2), PrintJobDownloaded (seq 3)
        let count = events.len();
        assert!(count >= 2);

        store.save_all(&job_id, &events).unwrap();

        let found = store.find_by_aggregate(&job_id).unwrap();
        assert_eq!(found.len(), count);
        assert_eq!(found[0].sequence_number, 1);
        assert_eq!(found[1].sequence_number, 2);
        if count >= 3 {
            assert_eq!(found[2].sequence_number, 3);
        }
    }

    #[test]
    fn test_sequence_numbering() {
        let conn = setup_test_db();
        let store = SqliteEventStore::new(conn);

        let agg = "agg-1";

        // First event → seq 1
        assert_eq!(store.next_sequence_number(agg).unwrap(), 1);

        // Manually insert seq 1 and 2
        {
            let c = store.conn.lock().unwrap();
            c.execute(
                "INSERT INTO events (aggregate_id, sequence_number, event_type, payload, timestamp)
                 VALUES (?1, 1, 'E1', '{}', ?2)",
                rusqlite::params![agg, 1_700_000_000i64],
            )
            .unwrap();
            c.execute(
                "INSERT INTO events (aggregate_id, sequence_number, event_type, payload, timestamp)
                 VALUES (?1, 2, 'E2', '{}', ?2)",
                rusqlite::params![agg, 1_700_000_001i64],
            )
            .unwrap();
        }

        // Next should be 3
        assert_eq!(store.next_sequence_number(agg).unwrap(), 3);
    }
}
