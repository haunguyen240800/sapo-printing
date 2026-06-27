use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::infrastructure::telemetry::metrics::MetricsError;
use crate::infrastructure::persistence::task_queue::QueueManager;

#[derive(Debug, Clone)]
pub struct JobMetrics {
    pub total_jobs: u64,
    pub pending: u64,
    pub queued: u64,
    pub downloaded: u64,
    pub submitted: u64,
    pub printing: u64,
    pub completed: u64,
    pub failed: u64,
    pub cancelled: u64,
    pub success_rate: f64,
}

#[derive(Debug, Clone)]
pub struct QueueMetrics {
    pub current_depth: usize,
    pub avg_wait_time_secs: f64,
}

#[derive(Debug, Clone)]
pub struct PrinterJobStats {
    pub printer_name: String,
    pub total_jobs: u64,
    pub completed_jobs: u64,
    pub utilization_percent: f64,
}

#[derive(Debug, Clone)]
pub struct PrinterMetrics {
    pub printers: Vec<PrinterJobStats>,
}

#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub avg_job_duration_secs: f64,
    pub p50_job_duration_secs: f64,
    pub p95_job_duration_secs: f64,
    pub p99_job_duration_secs: f64,
    pub avg_download_time_secs: f64,
    pub avg_render_time_secs: f64,
    pub avg_print_time_secs: f64,
}

#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub collected_at: i64,
    pub job_metrics: JobMetrics,
    pub queue_metrics: QueueMetrics,
    pub printer_metrics: PrinterMetrics,
    pub performance_metrics: PerformanceMetrics,
}

pub struct MetricsCollector {
    conn: Arc<Mutex<Connection>>,
    queue_manager: Arc<dyn QueueManager>,
}

impl MetricsCollector {
    pub fn new(conn: Arc<Mutex<Connection>>, queue_manager: Arc<dyn QueueManager>) -> Self {
        Self {
            conn,
            queue_manager,
        }
    }

    pub fn collect_metrics(&self) -> Result<MetricsSnapshot, MetricsError> {
        tracing::debug!(
            target = "sapo_printer::metrics",
            "MetricsCollector: acquiring database lock"
        );

        let conn = self.conn.lock().map_err(|e| {
            MetricsError::DatabaseError(format!("Failed to lock connection: {}", e))
        })?;

        tracing::debug!(
            target = "sapo_printer::metrics",
            "MetricsCollector: lock acquired, collecting metrics"
        );

        let job_metrics = self.collect_job_metrics(&conn)?;
        let queue_metrics = self.collect_queue_metrics(&conn)?;
        let printer_metrics = self.collect_printer_metrics(&conn)?;
        let performance_metrics = self.collect_performance_metrics(&conn)?;

        // Explicitly drop lock ASAP
        drop(conn);

        tracing::debug!(
            target = "sapo_printer::metrics",
            "MetricsCollector: lock released"
        );

        let collected_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        Ok(MetricsSnapshot {
            collected_at,
            job_metrics,
            queue_metrics,
            printer_metrics,
            performance_metrics,
        })
    }

    fn collect_job_metrics(&self, conn: &Connection) -> Result<JobMetrics, MetricsError> {
        let mut stmt = conn
            .prepare("SELECT status, COUNT(*) FROM print_jobs GROUP BY status")
            .map_err(|e| MetricsError::DatabaseError(format!("Failed to prepare query: {}", e)))?;

        let mut counts = std::collections::HashMap::new();
        let rows = stmt
            .query_map([], |row| {
                let status: String = row.get(0)?;
                let count: u64 = row.get(1)?;
                Ok((status, count))
            })
            .map_err(|e| MetricsError::DatabaseError(format!("Failed to query: {}", e)))?;

        for row in rows {
            let (status, count) =
                row.map_err(|e| MetricsError::DatabaseError(format!("Row error: {}", e)))?;
            counts.insert(status, count);
        }

        let pending = counts.get("PENDING").copied().unwrap_or(0);
        let queued = counts.get("QUEUED").copied().unwrap_or(0);
        let downloaded = counts.get("DOWNLOADED").copied().unwrap_or(0);
        let submitted = counts.get("SUBMITTED_TO_QUEUE").copied().unwrap_or(0);
        let printing = counts.get("PRINTING").copied().unwrap_or(0);
        let completed = counts.get("COMPLETED").copied().unwrap_or(0);
        let failed = counts.get("FAILED").copied().unwrap_or(0);
        let cancelled = counts.get("CANCELLED").copied().unwrap_or(0);

        let total_jobs: u64 = conn
            .query_row("SELECT COUNT(*) FROM print_jobs", [], |row| row.get(0))
            .map_err(|e| MetricsError::DatabaseError(format!("Failed to count total jobs: {}", e)))?;

        let terminal = completed + failed;
        let success_rate = if terminal > 0 {
            (completed as f64 / terminal as f64) * 100.0
        } else {
            0.0
        };

        Ok(JobMetrics {
            total_jobs,
            pending,
            queued,
            downloaded,
            submitted,
            printing,
            completed,
            failed,
            cancelled,
            success_rate,
        })
    }

    fn collect_queue_metrics(&self, conn: &Connection) -> Result<QueueMetrics, MetricsError> {
        let current_depth = self
            .queue_manager
            .queue_depth()
            .map_err(|e| MetricsError::QueueError(format!("{}", e)))?;

        let avg_wait_time_secs: f64 = conn
            .query_row(
                "SELECT COALESCE(AVG(CAST(e.timestamp AS FLOAT) - CAST(j.created_at AS FLOAT)), 0.0)
                 FROM print_jobs j
                 JOIN events e ON j.id = e.aggregate_id
                 WHERE e.event_type = 'PrintJobQueued'
                   AND e.timestamp >= j.created_at
                   AND e.sequence_number = (
                       SELECT MIN(e2.sequence_number)
                       FROM events e2
                       WHERE e2.aggregate_id = j.id
                         AND e2.event_type = 'PrintJobQueued'
                   )",
                [],
                |row| row.get(0),
            )
            .map_err(|e| MetricsError::DatabaseError(format!("Failed to query avg wait time: {}", e)))?;

        Ok(QueueMetrics {
            current_depth,
            avg_wait_time_secs,
        })
    }

    fn collect_printer_metrics(&self, conn: &Connection) -> Result<PrinterMetrics, MetricsError> {
        let mut stmt = conn
            .prepare(
                "SELECT printer_name,
                        COUNT(*) as total,
                        SUM(CASE WHEN status = 'COMPLETED' THEN 1 ELSE 0 END) as completed
                 FROM print_jobs
                 GROUP BY printer_name
                 ORDER BY printer_name",
            )
            .map_err(|e| MetricsError::DatabaseError(format!("Failed to prepare printer query: {}", e)))?;

        let printers = stmt
            .query_map([], |row| {
                let printer_name: String = row.get(0)?;
                let total_jobs: u64 = row.get(1)?;
                let completed_jobs: u64 = row.get(2)?;
                Ok((printer_name, total_jobs, completed_jobs))
            })
            .map_err(|e| MetricsError::DatabaseError(format!("Failed to query printers: {}", e)))?;

        let mut result = Vec::new();
        for row in printers {
            let (printer_name, total_jobs, completed_jobs) =
                row.map_err(|e| MetricsError::DatabaseError(format!("Row error: {}", e)))?;
            let utilization_percent = if total_jobs > 0 {
                (completed_jobs as f64 / total_jobs as f64) * 100.0
            } else {
                0.0
            };
            result.push(PrinterJobStats {
                printer_name,
                total_jobs,
                completed_jobs,
                utilization_percent,
            });
        }

        Ok(PrinterMetrics { printers: result })
    }

    fn collect_performance_metrics(
        &self,
        conn: &Connection,
    ) -> Result<PerformanceMetrics, MetricsError> {
        let durations = self.fetch_completed_durations(conn)?;

        let avg_job_duration_secs = if durations.is_empty() {
            0.0
        } else {
            durations.iter().sum::<f64>() / durations.len() as f64
        };

        let mut sorted = durations.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let p50_job_duration_secs = percentile(&sorted, 50.0);
        let p95_job_duration_secs = percentile(&sorted, 95.0);
        let p99_job_duration_secs = percentile(&sorted, 99.0);

        let avg_download_time_secs = self.fetch_step_duration(
            conn,
            "PrintJobQueued",
            "PrintJobDownloaded",
        )?;

        let avg_render_time_secs = self.fetch_step_duration(
            conn,
            "PrintJobDownloaded",
            "PrintJobSubmitted",
        )?;

        let avg_print_time_secs = self.fetch_step_duration(
            conn,
            "PrintJobPrinting",
            "PrintJobCompleted",
        )?;

        Ok(PerformanceMetrics {
            avg_job_duration_secs,
            p50_job_duration_secs,
            p95_job_duration_secs,
            p99_job_duration_secs,
            avg_download_time_secs,
            avg_render_time_secs,
            avg_print_time_secs,
        })
    }

    fn fetch_completed_durations(
        &self,
        conn: &Connection,
    ) -> Result<Vec<f64>, MetricsError> {
        let mut stmt = conn
            .prepare(
                "SELECT CAST(completed_at AS FLOAT) - CAST(created_at AS FLOAT)
                 FROM print_jobs
                 WHERE status = 'COMPLETED'
                   AND completed_at IS NOT NULL
                   AND completed_at >= created_at",
            )
            .map_err(|e| MetricsError::DatabaseError(format!("Failed to prepare durations query: {}", e)))?;

        let rows = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| MetricsError::DatabaseError(format!("Failed to query durations: {}", e)))?;

        let mut durations = Vec::new();
        for row in rows {
            let val: f64 =
                row.map_err(|e| MetricsError::DatabaseError(format!("Row error: {}", e)))?;
            durations.push(val);
        }
        Ok(durations)
    }

    fn fetch_step_duration(
        &self,
        conn: &Connection,
        from_event: &str,
        to_event: &str,
    ) -> Result<f64, MetricsError> {
        let result: f64 = conn
            .query_row(
                "SELECT COALESCE(AVG(CAST(e2.timestamp AS FLOAT) - CAST(e1.timestamp AS FLOAT)), 0.0)
                 FROM events e1
                 JOIN events e2 ON e1.aggregate_id = e2.aggregate_id
                     AND e2.event_type = ?2
                     AND e2.sequence_number = (
                         SELECT MIN(e3.sequence_number)
                         FROM events e3
                         WHERE e3.aggregate_id = e1.aggregate_id
                           AND e3.event_type = ?2
                           AND e3.sequence_number > e1.sequence_number
                     )
                 WHERE e1.event_type = ?1
                   AND e2.timestamp >= e1.timestamp",
                rusqlite::params![from_event, to_event],
                |row| row.get(0),
            )
            .map_err(|e| {
                MetricsError::DatabaseError(format!(
                    "Failed to query step duration {} -> {}: {}",
                    from_event, to_event, e
                ))
            })?;
        Ok(result)
    }
}

fn percentile(sorted_values: &[f64], p: f64) -> f64 {
    if sorted_values.is_empty() {
        return 0.0;
    }
    if sorted_values.len() == 1 {
        return sorted_values[0];
    }
    let rank = (p / 100.0) * (sorted_values.len() - 1) as f64;
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    let frac = rank - lower as f64;
    sorted_values[lower] * (1.0 - frac) + sorted_values[upper] * frac
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::PrintJob;
    use crate::infrastructure::persistence::sqlite::migrations::run_migrations;
    use crate::infrastructure::persistence::task_queue::QueueError;
    use crate::domain::models::JobId;

    struct MockQueueManager {
        depth: usize,
    }

    impl MockQueueManager {
        fn new(depth: usize) -> Self {
            Self { depth }
        }
    }

    impl QueueManager for MockQueueManager {
        fn push(&self, _job_id: &JobId) -> Result<(), QueueError> {
            Ok(())
        }
        fn pop(&self) -> Result<Option<PrintJob>, QueueError> {
            Ok(None)
        }
        fn requeue(&self, _job_id: &JobId, _delay_secs: u64) -> Result<(), QueueError> {
            Ok(())
        }
        fn queue_depth(&self) -> Result<usize, QueueError> {
            Ok(self.depth)
        }
    }

    fn setup() -> (Arc<Mutex<Connection>>, Arc<MockQueueManager>) {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();
        let conn = Arc::new(Mutex::new(conn));
        let qm = Arc::new(MockQueueManager::new(5));
        (conn, qm)
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
        let (conn, qm) = setup();
        {
            let c = conn.lock().unwrap();
            insert_job_with_status(&c, "00000000-0000-0000-0000-000000000001", "HP", "PENDING", 1000, None);
            insert_job_with_status(&c, "00000000-0000-0000-0000-000000000002", "HP", "QUEUED", 1000, None);
            insert_job_with_status(&c, "00000000-0000-0000-0000-000000000003", "HP", "DOWNLOADED", 1000, None);
            insert_job_with_status(&c, "00000000-0000-0000-0000-000000000004", "HP", "SUBMITTED_TO_QUEUE", 1000, None);
            insert_job_with_status(&c, "00000000-0000-0000-0000-000000000005", "HP", "PRINTING", 1000, None);
            insert_job_with_status(&c, "00000000-0000-0000-0000-000000000006", "HP", "COMPLETED", 1000, Some(2000));
            insert_job_with_status(&c, "00000000-0000-0000-0000-000000000007", "HP", "COMPLETED", 1000, Some(2000));
            insert_job_with_status(&c, "00000000-0000-0000-0000-000000000008", "HP", "FAILED", 1000, None);
            insert_job_with_status(&c, "00000000-0000-0000-0000-000000000009", "HP", "CANCELLED", 1000, None);
        }

        let collector = MetricsCollector::new(conn, qm);
        let snapshot = collector.collect_metrics().unwrap();

        assert_eq!(snapshot.job_metrics.total_jobs, 9);
        assert_eq!(snapshot.job_metrics.pending, 1);
        assert_eq!(snapshot.job_metrics.queued, 1);
        assert_eq!(snapshot.job_metrics.downloaded, 1);
        assert_eq!(snapshot.job_metrics.submitted, 1);
        assert_eq!(snapshot.job_metrics.printing, 1);
        assert_eq!(snapshot.job_metrics.completed, 2);
        assert_eq!(snapshot.job_metrics.failed, 1);
        assert_eq!(snapshot.job_metrics.cancelled, 1);
    }

    #[test]
    fn test_success_rate_calculation() {
        let (conn, qm) = setup();
        {
            let c = conn.lock().unwrap();
            for i in 0..8 {
                let id = format!("00000000-0000-0000-0000-{:012}", i + 1);
                insert_job_with_status(&c, &id, "HP", "COMPLETED", 1000, Some(2000));
            }
            for i in 0..2 {
                let id = format!("00000000-0000-0000-0000-{:012}", i + 100);
                insert_job_with_status(&c, &id, "HP", "FAILED", 1000, None);
            }
        }

        let collector = MetricsCollector::new(conn, qm);
        let snapshot = collector.collect_metrics().unwrap();

        assert_eq!(snapshot.job_metrics.completed, 8);
        assert_eq!(snapshot.job_metrics.failed, 2);
        assert!((snapshot.job_metrics.success_rate - 80.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_success_rate_zero_terminal_jobs() {
        let (conn, qm) = setup();
        {
            let c = conn.lock().unwrap();
            insert_job_with_status(&c, "00000000-0000-0000-0000-000000000001", "HP", "PENDING", 1000, None);
            insert_job_with_status(&c, "00000000-0000-0000-0000-000000000002", "HP", "QUEUED", 1000, None);
        }

        let collector = MetricsCollector::new(conn, qm);
        let snapshot = collector.collect_metrics().unwrap();

        assert!((snapshot.job_metrics.success_rate - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_queue_depth_from_mock() {
        let (conn, qm) = setup();

        let collector = MetricsCollector::new(conn, qm);
        let snapshot = collector.collect_metrics().unwrap();

        assert_eq!(snapshot.queue_metrics.current_depth, 5);
    }

    #[test]
    fn test_percentile_calculation() {
        let values: Vec<f64> = (1..=10).map(|i| i as f64).collect();
        let sorted = values;

        assert!((percentile(&sorted, 50.0) - 5.5).abs() < 0.01);
        assert!((percentile(&sorted, 95.0) - 9.55).abs() < 0.01);
        assert!((percentile(&sorted, 99.0) - 9.91).abs() < 0.01);
    }

    #[test]
    fn test_percentile_empty() {
        let empty: Vec<f64> = vec![];
        assert!((percentile(&empty, 50.0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_percentile_single_value() {
        let single = vec![42.0];
        assert!((percentile(&single, 50.0) - 42.0).abs() < f64::EPSILON);
        assert!((percentile(&single, 95.0) - 42.0).abs() < f64::EPSILON);
        assert!((percentile(&single, 99.0) - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_printer_utilization() {
        let (conn, qm) = setup();
        {
            let c = conn.lock().unwrap();
            insert_job_with_status(&c, "00000000-0000-0000-0000-000000000001", "PrinterA", "COMPLETED", 1000, Some(2000));
            insert_job_with_status(&c, "00000000-0000-0000-0000-000000000002", "PrinterA", "COMPLETED", 1000, Some(2000));
            insert_job_with_status(&c, "00000000-0000-0000-0000-000000000003", "PrinterA", "FAILED", 1000, None);
            insert_job_with_status(&c, "00000000-0000-0000-0000-000000000004", "PrinterB", "COMPLETED", 1000, Some(2000));
            insert_job_with_status(&c, "00000000-0000-0000-0000-000000000005", "PrinterB", "COMPLETED", 1000, Some(2000));
        }

        let collector = MetricsCollector::new(conn, qm);
        let snapshot = collector.collect_metrics().unwrap();

        let printers = &snapshot.printer_metrics.printers;
        assert_eq!(printers.len(), 2);

        let a = printers.iter().find(|p| p.printer_name == "PrinterA").unwrap();
        assert_eq!(a.total_jobs, 3);
        assert_eq!(a.completed_jobs, 2);
        assert!((a.utilization_percent - 66.666).abs() < 0.1);

        let b = printers.iter().find(|p| p.printer_name == "PrinterB").unwrap();
        assert_eq!(b.total_jobs, 2);
        assert_eq!(b.completed_jobs, 2);
        assert!((b.utilization_percent - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_step_duration_from_events() {
        let (conn, qm) = setup();
        {
            let c = conn.lock().unwrap();
            let job_id = "00000000-0000-0000-0000-000000000001";
            insert_job_with_status(&c, job_id, "HP", "COMPLETED", 1000, Some(2000));
            insert_event(&c, job_id, 1, "PrintJobCreated", 1000);
            insert_event(&c, job_id, 2, "PrintJobQueued", 1010);
            insert_event(&c, job_id, 3, "PrintJobDownloaded", 1030);
            insert_event(&c, job_id, 4, "PrintJobSubmitted", 1040);
            insert_event(&c, job_id, 5, "PrintJobPrinting", 1050);
            insert_event(&c, job_id, 6, "PrintJobCompleted", 1100);
        }

        let collector = MetricsCollector::new(conn, qm);
        let snapshot = collector.collect_metrics().unwrap();

        assert!((snapshot.performance_metrics.avg_download_time_secs - 20.0).abs() < f64::EPSILON);
        assert!((snapshot.performance_metrics.avg_render_time_secs - 10.0).abs() < f64::EPSILON);
        assert!((snapshot.performance_metrics.avg_print_time_secs - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_empty_database() {
        let (conn, qm) = setup();

        let collector = MetricsCollector::new(conn, qm);
        let snapshot = collector.collect_metrics().unwrap();

        assert_eq!(snapshot.job_metrics.total_jobs, 0);
        assert_eq!(snapshot.job_metrics.pending, 0);
        assert_eq!(snapshot.job_metrics.completed, 0);
        assert!((snapshot.job_metrics.success_rate - 0.0).abs() < f64::EPSILON);
        assert_eq!(snapshot.queue_metrics.current_depth, 5);
        assert!((snapshot.queue_metrics.avg_wait_time_secs - 0.0).abs() < f64::EPSILON);
        assert!(snapshot.printer_metrics.printers.is_empty());
        assert!((snapshot.performance_metrics.avg_job_duration_secs - 0.0).abs() < f64::EPSILON);
        assert!((snapshot.performance_metrics.p50_job_duration_secs - 0.0).abs() < f64::EPSILON);
        assert!((snapshot.performance_metrics.avg_download_time_secs - 0.0).abs() < f64::EPSILON);
    }
}
