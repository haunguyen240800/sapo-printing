use rusqlite::{Connection, OptionalExtension};

use crate::application::errors::Error;
use crate::application::ports::{MetricsPort, MetricsSnapshot};
use crate::infrastructure::configs::db::DbPool;
use crate::infrastructure::errors::InfrastructureError;

pub struct MetricsCollector {
    pool: DbPool,
}

impl MetricsCollector {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn collect_metrics(&self) -> Result<MetricsSnapshot, InfrastructureError> {
        tracing::debug!(
            target = "sapo_printer::metrics",
            "MetricsCollector: acquiring pooled connection"
        );

        let conn = self
            .pool
            .get()
            .map_err(|e| InfrastructureError::DatabaseError {
                reason: format!("Failed to acquire DB connection: {}", e),
            })?;

        tracing::debug!(
            target = "sapo_printer::metrics",
            "MetricsCollector: connection acquired, collecting metrics"
        );

        let (total_jobs, completed, failed) = self.fetch_job_counts(&conn)?;
        let last_print_at = self.fetch_last_print_at(&conn)?;

        drop(conn);

        tracing::debug!(
            target = "sapo_printer::metrics",
            "MetricsCollector: connection released"
        );

        Ok(MetricsSnapshot {
            total_jobs,
            completed,
            failed,
            last_print_at,
        })
    }

    fn fetch_job_counts(&self, conn: &Connection) -> Result<(u64, u64, u64), InfrastructureError> {
        let mut stmt = conn
            .prepare("SELECT status, COUNT(*) FROM print_jobs GROUP BY status")
            .map_err(|e| InfrastructureError::DatabaseError {
                reason: format!("Failed to prepare query: {}", e),
            })?;

        let mut counts = std::collections::HashMap::new();
        let rows = stmt
            .query_map([], |row| {
                let status: String = row.get(0)?;
                let count: u64 = row.get(1)?;
                Ok((status, count))
            })
            .map_err(|e| InfrastructureError::DatabaseError {
                reason: format!("Failed to query: {}", e),
            })?;

        for row in rows {
            let (status, count) = row.map_err(|e| InfrastructureError::DatabaseError {
                reason: format!("Row error: {}", e),
            })?;
            counts.insert(status, count);
        }

        let completed = counts.get("COMPLETED").copied().unwrap_or(0);
        let failed = counts.get("FAILED").copied().unwrap_or(0);

        let total_jobs: u64 = conn
            .query_row("SELECT COUNT(*) FROM print_jobs", [], |row| row.get(0))
            .map_err(|e| InfrastructureError::DatabaseError {
                reason: format!("Failed to count total jobs: {}", e),
            })?;

        Ok((total_jobs, completed, failed))
    }

    fn fetch_last_print_at(&self, conn: &Connection) -> Result<i64, InfrastructureError> {
        let result: Option<i64> = conn
            .query_row(
                "SELECT timestamp
                 FROM events
                 WHERE event_type = 'PrintJobCompleted'
                 ORDER BY timestamp DESC, sequence_number DESC
                 LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| InfrastructureError::DatabaseError {
                reason: format!("Failed to query last print timestamp: {}", e),
            })?;
        Ok(result.unwrap_or(0))
    }
}

impl MetricsPort for MetricsCollector {
    fn collect(&self) -> Result<MetricsSnapshot, Error> {
        self.collect_metrics().map_err(Error::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::configs::db::run_migrations;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn setup() -> DbPool {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let thread_id = std::thread::current().id();
        let path = std::env::temp_dir().join(format!("sapo_metrics_test_{nanos}_{thread_id:?}.db"));
        let pool = DbPool::new(path.to_str().unwrap()).unwrap();
        {
            let mut conn = pool.get().unwrap();
            run_migrations(&mut *conn).unwrap();
        }
        pool
    }

    fn insert_job_with_status(
        conn: &Connection,
        id: &str,
        printer_name: &str,
        status: &str,
        created_at: i64,
        completed_at: Option<i64>,
    ) {
        conn.execute(
            "INSERT INTO print_jobs (id, printer_name, document_url, status, retry_count, created_at, updated_at, completed_at)
             VALUES (?1, ?2, 'https://example.com/doc.pdf', ?3, 0, ?4, ?4, ?5)",
            rusqlite::params![id, printer_name, status, created_at, completed_at],
        )
        .unwrap();
    }

    fn insert_event(
        conn: &Connection,
        aggregate_id: &str,
        sequence_number: i64,
        event_type: &str,
        timestamp: i64,
    ) {
        conn.execute(
            "INSERT INTO events (aggregate_id, sequence_number, event_type, payload, timestamp)
             VALUES (?1, ?2, ?3, '{}', ?4)",
            rusqlite::params![aggregate_id, sequence_number, event_type, timestamp],
        )
        .unwrap();
    }

    #[test]
    fn test_job_metrics_counting() {
        let pool = setup();
        {
            let c = pool.get().unwrap();
            insert_job_with_status(
                &c,
                "00000000-0000-0000-0000-000000000001",
                "HP",
                "PENDING",
                1000,
                None,
            );
            insert_job_with_status(
                &c,
                "00000000-0000-0000-0000-000000000006",
                "HP",
                "COMPLETED",
                1000,
                Some(2000),
            );
            insert_job_with_status(
                &c,
                "00000000-0000-0000-0000-000000000007",
                "HP",
                "COMPLETED",
                1000,
                Some(2000),
            );
            insert_job_with_status(
                &c,
                "00000000-0000-0000-0000-000000000008",
                "HP",
                "FAILED",
                1000,
                None,
            );
        }

        let collector = MetricsCollector::new(pool.clone());
        let snapshot = collector.collect_metrics().unwrap();

        assert_eq!(snapshot.total_jobs, 4);
        assert_eq!(snapshot.completed, 2);
        assert_eq!(snapshot.failed, 1);
    }

    #[test]
    fn test_last_print_at_from_events() {
        let pool = setup();
        {
            let c = pool.get().unwrap();

            let older = "00000000-0000-0000-0000-000000000001";
            insert_job_with_status(&c, older, "HP", "COMPLETED", 1000, Some(1100));
            insert_event(&c, older, 1, "PrintJobPrinting", 1050);
            insert_event(&c, older, 2, "PrintJobCompleted", 1100);

            let newer = "00000000-0000-0000-0000-000000000002";
            insert_job_with_status(&c, newer, "HP", "COMPLETED", 2000, Some(2200));
            insert_event(&c, newer, 1, "PrintJobPrinting", 2150);
            insert_event(&c, newer, 2, "PrintJobCompleted", 2200);
        }

        let collector = MetricsCollector::new(pool.clone());
        let snapshot = collector.collect_metrics().unwrap();

        // Timestamp of the most recent PrintJobCompleted event
        assert_eq!(snapshot.last_print_at, 2200);
    }

    #[test]
    fn test_empty_database() {
        let pool = setup();

        let collector = MetricsCollector::new(pool.clone());
        let snapshot = collector.collect_metrics().unwrap();

        assert_eq!(snapshot.total_jobs, 0);
        assert_eq!(snapshot.completed, 0);
        assert_eq!(snapshot.failed, 0);
        assert_eq!(snapshot.last_print_at, 0);
    }
}
