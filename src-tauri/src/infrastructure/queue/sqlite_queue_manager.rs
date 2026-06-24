use crate::domain::print_job::aggregate::PrintJob;
use crate::domain::print_job::value_objects::{JobId, PrintStatus};
use crate::infrastructure::queue::queue_manager::{QueueError, QueueManager};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct SqliteQueueManager {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteQueueManager {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }
}

impl QueueManager for SqliteQueueManager {
    fn push(&self, job_id: &JobId) -> Result<(), QueueError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let id_str = job_id.to_string();

        // Check current status
        let result: rusqlite::Result<String> = conn.query_row(
            "SELECT status FROM print_jobs WHERE id = ?1",
            [&id_str],
            |row| row.get(0),
        );

        match result {
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(QueueError::JobNotFound(id_str))
            }
            Err(e) => return Err(QueueError::RepositoryError(e.to_string())),
            Ok(status) if status != "Pending" => {
                return Err(QueueError::InvalidState(format!(
                    "Expected Pending, got {}",
                    status
                )))
            }
            Ok(_) => {} // Pending — proceed
        }

        // Update to Queued
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        conn.execute(
            "UPDATE print_jobs SET status = 'Queued', updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, id_str],
        )
        .map_err(|e| QueueError::RepositoryError(e.to_string()))?;

        Ok(())
    }

    fn pop(&self) -> Result<Option<PrintJob>, QueueError> {
        let mut conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());

        // Use transaction to avoid race condition between workers
        let tx = conn
            .transaction()
            .map_err(|e| QueueError::RepositoryError(e.to_string()))?;

        let job_opt = {
            let mut stmt = tx
                .prepare(
                    "SELECT id, printer_name, document_url, retry_count
                     FROM print_jobs WHERE status = 'Queued'
                     ORDER BY created_at ASC LIMIT 1",
                )
                .map_err(|e| QueueError::RepositoryError(e.to_string()))?;

            let result = stmt.query_row([], |row| {
                let id_str: String = row.get(0)?;
                let printer_name: String = row.get(1)?;
                let document_url: String = row.get(2)?;
                let retry_count: i64 = row.get(3)?;

                let id: JobId = id_str.parse().map_err(|e: uuid::Error| {
                    rusqlite::Error::InvalidColumnType(
                        0,
                        e.to_string(),
                        rusqlite::types::Type::Text,
                    )
                })?;

                // Reconstruct with status = Pending (since we're about to update it to Pending)
                Ok(PrintJob::reconstruct(
                    id,
                    PrintStatus::Pending,
                    retry_count as u32,
                    document_url,
                    printer_name,
                ))
            });

            match result {
                Ok(job) => Some(job),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(e) => return Err(QueueError::RepositoryError(e.to_string())),
            }
        };

        if let Some(job) = &job_opt {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            tx.execute(
                "UPDATE print_jobs SET status = 'Pending', updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now, job.id().to_string()],
            )
            .map_err(|e| QueueError::RepositoryError(e.to_string()))?;
        }

        tx.commit()
            .map_err(|e| QueueError::RepositoryError(e.to_string()))?;

        Ok(job_opt)
    }

    fn requeue(&self, job_id: &JobId, delay_secs: u64) -> Result<(), QueueError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let id_str = job_id.to_string();

        // Log delay_secs but don't use it (Story 3.6 will handle)
        if delay_secs > 0 {
            tracing::debug!(
                "Requeue requested with delay_secs={}, ignoring for now",
                delay_secs
            );
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        conn.execute(
            "UPDATE print_jobs SET status = 'Queued', updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, id_str],
        )
        .map_err(|e| QueueError::RepositoryError(e.to_string()))?;

        Ok(())
    }

    fn queue_depth(&self) -> Result<usize, QueueError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM print_jobs WHERE status = 'Queued'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| QueueError::RepositoryError(e.to_string()))?;

        Ok(count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database::migrations::run_migrations;

    fn setup() -> (Arc<Mutex<Connection>>, SqliteQueueManager) {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();
        let arc = Arc::new(Mutex::new(conn));
        let mgr = SqliteQueueManager::new(arc.clone());
        (arc, mgr)
    }

    fn insert_job(conn: &Arc<Mutex<Connection>>, job_id: &str, status: &str) {
        let c = conn.lock().unwrap();
        let now = 1_700_000_000i64;
        c.execute(
            "INSERT INTO print_jobs (id, printer_name, document_url, status, retry_count, created_at, updated_at)
             VALUES (?1, 'HP', 'https://s3.example.com/doc.pdf', ?2, 0, ?3, ?3)",
            rusqlite::params![job_id, status, now],
        )
        .unwrap();
    }

    fn insert_job_with_timestamp(
        conn: &Arc<Mutex<Connection>>,
        job_id: &str,
        status: &str,
        created_at: i64,
    ) {
        let c = conn.lock().unwrap();
        c.execute(
            "INSERT INTO print_jobs (id, printer_name, document_url, status, retry_count, created_at, updated_at)
             VALUES (?1, 'HP', 'https://s3.example.com/doc.pdf', ?2, 0, ?3, ?3)",
            rusqlite::params![job_id, status, created_at],
        )
        .unwrap();
    }

    #[test]
    fn test_push_pending_to_queued() {
        let (conn, mgr) = setup();
        let job_id = JobId::new();
        insert_job(&conn, &job_id.to_string(), "Pending");

        let result = mgr.push(&job_id);
        assert!(result.is_ok());

        let c = conn.lock().unwrap();
        let status: String = c
            .query_row(
                "SELECT status FROM print_jobs WHERE id = ?1",
                [job_id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "Queued");
    }

    #[test]
    fn test_push_non_pending_returns_invalid_state() {
        let (conn, mgr) = setup();
        let job_id = JobId::new();
        insert_job(&conn, &job_id.to_string(), "Queued");

        let result = mgr.push(&job_id);
        assert!(matches!(result, Err(QueueError::InvalidState(_))));
    }

    #[test]
    fn test_push_nonexistent_job_returns_not_found() {
        let (_conn, mgr) = setup();
        let job_id = JobId::new();

        let result = mgr.push(&job_id);
        assert!(matches!(result, Err(QueueError::JobNotFound(_))));
    }

    #[test]
    fn test_pop_returns_oldest_queued() {
        let (conn, mgr) = setup();
        let job_id1 = JobId::new();
        let job_id2 = JobId::new();

        insert_job_with_timestamp(&conn, &job_id1.to_string(), "Queued", 1_700_000_000);
        insert_job_with_timestamp(&conn, &job_id2.to_string(), "Queued", 1_700_000_100);

        let result = mgr.pop().unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().id(), &job_id1);
    }

    #[test]
    fn test_pop_empty_queue_returns_none() {
        let (_conn, mgr) = setup();

        let result = mgr.pop().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_pop_sets_status_to_pending() {
        let (conn, mgr) = setup();
        let job_id = JobId::new();
        insert_job(&conn, &job_id.to_string(), "Queued");

        mgr.pop().unwrap();

        let c = conn.lock().unwrap();
        let status: String = c
            .query_row(
                "SELECT status FROM print_jobs WHERE id = ?1",
                [job_id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "Pending");
    }

    #[test]
    fn test_requeue_sets_status_to_queued() {
        let (conn, mgr) = setup();
        let job_id = JobId::new();
        insert_job(&conn, &job_id.to_string(), "Pending");

        let result = mgr.requeue(&job_id, 0);
        assert!(result.is_ok());

        let c = conn.lock().unwrap();
        let status: String = c
            .query_row(
                "SELECT status FROM print_jobs WHERE id = ?1",
                [job_id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "Queued");
    }

    #[test]
    fn test_queue_depth_counts_queued() {
        let (conn, mgr) = setup();
        let job_id1 = JobId::new();
        let job_id2 = JobId::new();
        let job_id3 = JobId::new();
        let job_id4 = JobId::new();
        let job_id5 = JobId::new();

        insert_job(&conn, &job_id1.to_string(), "Queued");
        insert_job(&conn, &job_id2.to_string(), "Queued");
        insert_job(&conn, &job_id3.to_string(), "Queued");
        insert_job(&conn, &job_id4.to_string(), "Pending");
        insert_job(&conn, &job_id5.to_string(), "Pending");

        let depth = mgr.queue_depth().unwrap();
        assert_eq!(depth, 3);
    }

    #[test]
    fn test_queue_depth_empty() {
        let (_conn, mgr) = setup();

        let depth = mgr.queue_depth().unwrap();
        assert_eq!(depth, 0);
    }

    #[test]
    fn test_fifo_ordering() {
        let (conn, mgr) = setup();
        let job_id1 = JobId::new();
        let job_id2 = JobId::new();
        let job_id3 = JobId::new();

        insert_job_with_timestamp(&conn, &job_id1.to_string(), "Queued", 1_700_000_000);
        insert_job_with_timestamp(&conn, &job_id2.to_string(), "Queued", 1_700_000_050);
        insert_job_with_timestamp(&conn, &job_id3.to_string(), "Queued", 1_700_000_100);

        let popped1 = mgr.pop().unwrap().unwrap();
        assert_eq!(popped1.id(), &job_id1);

        let popped2 = mgr.pop().unwrap().unwrap();
        assert_eq!(popped2.id(), &job_id2);

        let popped3 = mgr.pop().unwrap().unwrap();
        assert_eq!(popped3.id(), &job_id3);
    }
}
