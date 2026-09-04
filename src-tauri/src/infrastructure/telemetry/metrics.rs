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
                let count: i64 = row.get(1)?;
                Ok((status, count as u64))
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
        let failed = counts.get("FAILED").copied().unwrap_or(0)
            + counts.get("CANCELLED").copied().unwrap_or(0);

        let total_jobs: i64 = conn
            .query_row("SELECT COUNT(*) FROM print_jobs", [], |row| row.get(0))
            .map_err(|e| InfrastructureError::DatabaseError {
                reason: format!("Failed to count total jobs: {}", e),
            })?;

        Ok((total_jobs as u64, completed, failed))
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
