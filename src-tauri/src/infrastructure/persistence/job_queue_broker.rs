use crate::application::ports::{QueueError, QueuePort};
use crate::domain::print_job::{PrintJob, PrintJobId, PrintStatus, PrinterId};
use crate::infrastructure::configs::db::{DbPool, SqliteConn};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct JobQueueBroker {
    pool: DbPool,
}

impl JobQueueBroker {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    fn acquire(&self) -> Result<SqliteConn, QueueError> {
        self.pool.get().map_err(|e| {
            QueueError::RepositoryError(format!("Failed to acquire DB connection: {}", e))
        })
    }
}

impl QueuePort for JobQueueBroker {
    fn push(&self, job_id: &PrintJobId) -> Result<(), QueueError> {
        let conn = self.acquire()?;
        let id_str = job_id.to_string();

        tracing::info!(
            target = "sapo_printer::queue_manager",
            job_id = %job_id,
            "JobQueueBroker: push() called"
        );

        // Note: SQLite's default isolation level (SERIALIZABLE) plus pooled
        // connection isolation prevents race conditions between push() and pop().

        // Check current status
        let result: rusqlite::Result<String> = conn.query_row(
            "SELECT status FROM print_jobs WHERE id = ?1",
            [&id_str],
            |row| row.get(0),
        );

        match result {
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                return Err(QueueError::JobNotFound(id_str));
            }
            Err(e) => return Err(QueueError::RepositoryError(e.to_string())),
            Ok(status) if status != PrintStatus::Pending.to_db_string() => {
                return Err(QueueError::InvalidState(format!(
                    "Expected {}, got {}",
                    PrintStatus::Pending.to_db_string(),
                    status
                )));
            }
            Ok(_) => {} // PENDING â€” proceed
        }

        // Update to QUEUED with scheduled_at = now (immediate execution)
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let rows_affected = conn.execute(
            "UPDATE print_jobs SET status = ?1, scheduled_at = ?2, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![PrintStatus::Queued.to_db_string(), now, id_str],
        )
        .map_err(|e| QueueError::RepositoryError(e.to_string()))?;

        tracing::info!(
            target = "sapo_printer::queue_manager",
            job_id = %job_id,
            rows_affected = rows_affected,
            "JobQueueBroker: job status updated to Queued"
        );

        Ok(())
    }

    fn pop(&self) -> Result<Option<PrintJob>, QueueError> {
        let mut conn = self.acquire()?;

        tracing::debug!(
            target = "sapo_printer::queue_manager",
            "JobQueueBroker: pop() called"
        );

        // Get current timestamp
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Use transaction to avoid race condition between workers
        let tx = conn
            .transaction()
            .map_err(|e| QueueError::RepositoryError(e.to_string()))?;

        let job_opt = {
            let mut stmt = tx
                .prepare(
                    "SELECT id, printer_name, document_url, retry_count, output_path, settings_json
                     FROM print_jobs
                     WHERE status = ?1
                       AND (scheduled_at IS NULL OR scheduled_at <= ?2)
                     ORDER BY created_at ASC
                     LIMIT 1",
                )
                .map_err(|e| QueueError::RepositoryError(e.to_string()))?;

            let result = stmt.query_row(
                rusqlite::params![PrintStatus::Queued.to_db_string(), now],
                |row| {
                    let id_str: String = row.get(0)?;
                    let printer_id_raw: String = row.get(1)?;
                    let document_url: String = row.get(2)?;
                    let retry_count: i64 = row.get(3)?;
                    let output_path: Option<String> = row.get(4)?;
                    let settings_json: Option<String> = row.get(5)?;

                    let id: PrintJobId = id_str.parse().map_err(|e: uuid::Error| {
                        rusqlite::Error::InvalidColumnType(
                            0,
                            e.to_string(),
                            rusqlite::types::Type::Text,
                        )
                    })?;

                    let settings = settings_json
                        .as_deref()
                        .and_then(|s| serde_json::from_str(s).ok())
                        .unwrap_or_default();

                    // Reconstruct with status = Pending (since we're about to update it to Pending)
                    Ok(PrintJob::reconstruct(
                        id,
                        PrintStatus::Pending,
                        retry_count as u32,
                        document_url,
                        PrinterId::new(printer_id_raw),
                        0,    // created_at not needed for queue pop
                        None, // completed_at not needed for queue pop
                        None, // error_message not needed for queue pop
                        output_path,
                        settings,
                    ))
                },
            );

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

            tracing::info!(
                target = "sapo_printer::queue_manager",
                job_id = %job.id(),
                "JobQueueBroker: popped job, updating status to PENDING"
            );

            tx.execute(
                "UPDATE print_jobs SET status = ?1, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![
                    PrintStatus::Pending.to_db_string(),
                    now,
                    job.id().to_string()
                ],
            )
            .map_err(|e| QueueError::RepositoryError(e.to_string()))?;
        } else {
            tracing::trace!(
                target = "sapo_printer::queue_manager",
                "JobQueueBroker: no jobs available in queue"
            );
        }

        tx.commit()
            .map_err(|e| QueueError::RepositoryError(e.to_string()))?;

        Ok(job_opt)
    }

    fn requeue(&self, job_id: &PrintJobId, delay_secs: u64) -> Result<(), QueueError> {
        let conn = self.acquire()?;
        let id_str = job_id.to_string();

        // Calculate scheduled_at timestamp (now + delay_secs)
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let scheduled_at = now.checked_add(delay_secs as i64).unwrap_or(i64::MAX);

        tracing::info!(
            "Requeue job {} with {}s delay (scheduled_at={})",
            job_id,
            delay_secs,
            scheduled_at
        );

        conn.execute(
            "UPDATE print_jobs SET status = ?1, scheduled_at = ?2, updated_at = ?3 WHERE id = ?4",
            rusqlite::params![
                PrintStatus::Queued.to_db_string(),
                scheduled_at,
                now,
                id_str
            ],
        )
        .map_err(|e| QueueError::RepositoryError(e.to_string()))?;

        Ok(())
    }

    fn queue_depth(&self) -> Result<usize, QueueError> {
        let conn = self.acquire()?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM print_jobs WHERE status = ?1",
                [PrintStatus::Queued.to_db_string()],
                |row| row.get(0),
            )
            .map_err(|e| QueueError::RepositoryError(e.to_string()))?;

        Ok(count as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::configs::db::run_migrations;

    /// Build a unique temp-file-backed DbPool for tests. r2d2_sqlite cannot
    /// share an in-memory database across pooled connections, so we use a
    /// per-test temp file.
    fn setup() -> (DbPool, JobQueueBroker, std::path::PathBuf) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let thread_id = std::thread::current().id();
        let path = std::env::temp_dir().join(format!("sapo_queue_test_{nanos}_{thread_id:?}.db"));
        let pool = DbPool::new(path.to_str().unwrap()).unwrap();
        {
            let mut conn = pool.get().unwrap();
            run_migrations(&mut *conn).unwrap();
        }
        let mgr = JobQueueBroker::new(pool.clone());
        (pool, mgr, path)
    }

    fn cleanup(path: &std::path::PathBuf) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }

    fn insert_job(pool: &DbPool, job_id: &str, status: &str) {
        let conn = pool.get().unwrap();
        let now = 1_700_000_000i64;
        conn.execute(
            "INSERT INTO print_jobs (id, printer_name, document_url, status, retry_count, created_at, updated_at)
             VALUES (?1, 'HP', 'https://s3.example.com/doc.pdf', ?2, 0, ?3, ?3)",
            rusqlite::params![job_id, status, now],
        )
        .unwrap();
    }

    fn insert_job_with_timestamp(pool: &DbPool, job_id: &str, status: &str, created_at: i64) {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO print_jobs (id, printer_name, document_url, status, retry_count, created_at, updated_at)
             VALUES (?1, 'HP', 'https://s3.example.com/doc.pdf', ?2, 0, ?3, ?3)",
            rusqlite::params![job_id, status, created_at],
        )
        .unwrap();
    }

    #[test]
    fn test_push_pending_to_queued() {
        let (pool, mgr, path) = setup();
        let job_id = PrintJobId::new();
        insert_job(&pool, &job_id.to_string(), "PENDING");

        let result = mgr.push(&job_id);
        assert!(result.is_ok());

        let conn = pool.get().unwrap();
        let status: String = conn
            .query_row(
                "SELECT status FROM print_jobs WHERE id = ?1",
                [job_id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "QUEUED");
        drop(conn);
        drop(pool);
        cleanup(&path);
    }

    #[test]
    fn test_push_non_pending_returns_invalid_state() {
        let (pool, mgr, path) = setup();
        let job_id = PrintJobId::new();
        insert_job(&pool, &job_id.to_string(), "QUEUED");

        let result = mgr.push(&job_id);
        assert!(matches!(result, Err(QueueError::InvalidState(_))));
        drop(pool);
        cleanup(&path);
    }

    #[test]
    fn test_push_nonexistent_job_returns_not_found() {
        let (pool, mgr, path) = setup();
        let job_id = PrintJobId::new();

        let result = mgr.push(&job_id);
        assert!(matches!(result, Err(QueueError::JobNotFound(_))));
        drop(pool);
        cleanup(&path);
    }

    #[test]
    fn test_pop_returns_oldest_queued() {
        let (pool, mgr, path) = setup();
        let job_id1 = PrintJobId::new();
        let job_id2 = PrintJobId::new();

        insert_job_with_timestamp(&pool, &job_id1.to_string(), "QUEUED", 1_700_000_000);
        insert_job_with_timestamp(&pool, &job_id2.to_string(), "QUEUED", 1_700_000_100);

        let result = mgr.pop().unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().id(), &job_id1);
        drop(pool);
        cleanup(&path);
    }

    #[test]
    fn test_pop_empty_queue_returns_none() {
        let (pool, mgr, path) = setup();

        let result = mgr.pop().unwrap();
        assert!(result.is_none());
        drop(pool);
        cleanup(&path);
    }

    #[test]
    fn test_pop_sets_status_to_pending() {
        let (pool, mgr, path) = setup();
        let job_id = PrintJobId::new();
        insert_job(&pool, &job_id.to_string(), "QUEUED");

        mgr.pop().unwrap();

        let conn = pool.get().unwrap();
        let status: String = conn
            .query_row(
                "SELECT status FROM print_jobs WHERE id = ?1",
                [job_id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "PENDING");
        drop(conn);
        drop(pool);
        cleanup(&path);
    }

    #[test]
    fn test_requeue_sets_status_to_queued() {
        let (pool, mgr, path) = setup();
        let job_id = PrintJobId::new();
        insert_job(&pool, &job_id.to_string(), "PENDING");

        let result = mgr.requeue(&job_id, 0);
        assert!(result.is_ok());

        let conn = pool.get().unwrap();
        let status: String = conn
            .query_row(
                "SELECT status FROM print_jobs WHERE id = ?1",
                [job_id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "QUEUED");
        drop(conn);
        drop(pool);
        cleanup(&path);
    }

    #[test]
    fn test_queue_depth_counts_queued() {
        let (pool, mgr, path) = setup();
        let job_id1 = PrintJobId::new();
        let job_id2 = PrintJobId::new();
        let job_id3 = PrintJobId::new();
        let job_id4 = PrintJobId::new();
        let job_id5 = PrintJobId::new();

        insert_job(&pool, &job_id1.to_string(), "QUEUED");
        insert_job(&pool, &job_id2.to_string(), "QUEUED");
        insert_job(&pool, &job_id3.to_string(), "QUEUED");
        insert_job(&pool, &job_id4.to_string(), "PENDING");
        insert_job(&pool, &job_id5.to_string(), "PENDING");

        let depth = mgr.queue_depth().unwrap();
        assert_eq!(depth, 3);
        drop(pool);
        cleanup(&path);
    }

    #[test]
    fn test_queue_depth_empty() {
        let (pool, mgr, path) = setup();

        let depth = mgr.queue_depth().unwrap();
        assert_eq!(depth, 0);
        drop(pool);
        cleanup(&path);
    }

    #[test]
    fn test_fifo_ordering() {
        let (pool, mgr, path) = setup();
        let job_id1 = PrintJobId::new();
        let job_id2 = PrintJobId::new();
        let job_id3 = PrintJobId::new();

        insert_job_with_timestamp(&pool, &job_id1.to_string(), "QUEUED", 1_700_000_000);
        insert_job_with_timestamp(&pool, &job_id2.to_string(), "QUEUED", 1_700_000_050);
        insert_job_with_timestamp(&pool, &job_id3.to_string(), "QUEUED", 1_700_000_100);

        let popped1 = mgr.pop().unwrap().unwrap();
        assert_eq!(popped1.id(), &job_id1);

        let popped2 = mgr.pop().unwrap().unwrap();
        assert_eq!(popped2.id(), &job_id2);

        let popped3 = mgr.pop().unwrap().unwrap();
        assert_eq!(popped3.id(), &job_id3);
        drop(pool);
        cleanup(&path);
    }

    #[test]
    fn test_pop_respects_scheduled_at() {
        let (pool, mgr, path) = setup();
        let job_id1 = PrintJobId::new();
        let job_id2 = PrintJobId::new();

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        {
            let conn = pool.get().unwrap();
            // Job 1: scheduled in the past (should be popped)
            conn.execute(
                "INSERT INTO print_jobs (id, printer_name, document_url, status, retry_count, created_at, updated_at, scheduled_at)
                 VALUES (?1, 'HP', 'https://s3.example.com/doc.pdf', 'QUEUED', 0, ?2, ?2, ?3)",
                rusqlite::params![job_id1.to_string(), now, now - 10],
            )
            .unwrap();

            // Job 2: scheduled in the future (should NOT be popped)
            conn.execute(
                "INSERT INTO print_jobs (id, printer_name, document_url, status, retry_count, created_at, updated_at, scheduled_at)
                 VALUES (?1, 'HP', 'https://s3.example.com/doc.pdf', 'QUEUED', 0, ?2, ?2, ?3)",
                rusqlite::params![job_id2.to_string(), now, now + 100],
            )
            .unwrap();
        }

        // Pop should return job1 only
        let popped = mgr.pop().unwrap();
        assert!(popped.is_some());
        assert_eq!(popped.unwrap().id(), &job_id1);

        // Second pop should return None (job2 not ready yet)
        let popped2 = mgr.pop().unwrap();
        assert!(popped2.is_none());
        drop(pool);
        cleanup(&path);
    }

    #[test]
    fn test_pop_restores_settings_snapshot() {
        use crate::domain::print_job::{PaperSize, PrintJobSettings};

        let (pool, mgr, path) = setup();
        let job_id = PrintJobId::new();

        let mut settings = PrintJobSettings::default();
        settings.paper_size = PaperSize::Cm10x15;
        settings.orientation = "LANDSCAPE".to_string();
        settings.color_mode = "GRAY".to_string();
        settings.copies = 3;
        let settings_json = serde_json::to_string(&settings).unwrap();

        {
            let conn = pool.get().unwrap();
            let now = 1_700_000_000i64;
            conn.execute(
                "INSERT INTO print_jobs (id, printer_name, document_url, status, retry_count, created_at, updated_at, settings_json)
                 VALUES (?1, 'HP', 'https://s3.example.com/doc.pdf', 'QUEUED', 0, ?2, ?2, ?3)",
                rusqlite::params![job_id.to_string(), now, settings_json],
            )
            .unwrap();
        }

        let popped = mgr.pop().unwrap().unwrap();
        assert_eq!(popped.settings, settings);
        drop(pool);
        cleanup(&path);
    }

    #[test]
    fn test_pop_null_settings_falls_back_to_default() {
        use crate::domain::print_job::PrintJobSettings;

        let (pool, mgr, path) = setup();
        let job_id = PrintJobId::new();
        // insert_job does not set settings_json → NULL
        insert_job(&pool, &job_id.to_string(), "QUEUED");

        let popped = mgr.pop().unwrap().unwrap();
        assert_eq!(popped.settings, PrintJobSettings::default());
        drop(pool);
        cleanup(&path);
    }

    #[test]
    fn test_requeue_with_delay() {
        let (pool, mgr, path) = setup();
        let job_id = PrintJobId::new();
        insert_job(&pool, &job_id.to_string(), "FAILED");

        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Requeue with 10s delay
        let result = mgr.requeue(&job_id, 10);
        assert!(result.is_ok());

        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Verify scheduled_at is set to now + 10s
        let conn = pool.get().unwrap();
        let (status, scheduled_at): (String, i64) = conn
            .query_row(
                "SELECT status, scheduled_at FROM print_jobs WHERE id = ?1",
                [job_id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(status, "QUEUED");
        assert!(scheduled_at >= before + 10);
        assert!(scheduled_at <= after + 10);
        drop(conn);
        drop(pool);
        cleanup(&path);
    }
}


