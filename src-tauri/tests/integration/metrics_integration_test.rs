//! Integration tests for metrics collection (Story 4.5).

use rusqlite::Connection;
use std::sync::{Arc, Mutex};

use sapo_printer::domain::print_job::aggregate::PrintJob;
use sapo_printer::domain::print_job::value_objects::JobId;
use sapo_printer::infrastructure::database::migrations::run_migrations;
use sapo_printer::infrastructure::metrics::collector::MetricsCollector;
use sapo_printer::infrastructure::queue::{QueueError, QueueManager};

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
    let qm = Arc::new(MockQueueManager::new(3));
    (conn, qm)
}

fn insert_job(
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
fn test_job_metrics_with_various_statuses() {
    let (conn, qm) = setup();
    {
        let c = conn.lock().unwrap();
        insert_job(&c, "00000000-0000-0000-0000-000000000001", "HP", "PENDING", 1000, None);
        insert_job(&c, "00000000-0000-0000-0000-000000000002", "HP", "QUEUED", 1000, None);
        insert_job(&c, "00000000-0000-0000-0000-000000000003", "HP", "DOWNLOADED", 1000, None);
        insert_job(&c, "00000000-0000-0000-0000-000000000004", "HP", "COMPLETED", 1000, Some(2000));
        insert_job(&c, "00000000-0000-0000-0000-000000000005", "HP", "FAILED", 1000, None);
    }

    let collector = MetricsCollector::new(conn, qm);
    let snapshot = collector.collect_metrics().unwrap();

    assert_eq!(snapshot.job_metrics.total_jobs, 5);
    assert_eq!(snapshot.job_metrics.pending, 1);
    assert_eq!(snapshot.job_metrics.queued, 1);
    assert_eq!(snapshot.job_metrics.downloaded, 1);
    assert_eq!(snapshot.job_metrics.completed, 1);
    assert_eq!(snapshot.job_metrics.failed, 1);
    assert!((snapshot.job_metrics.success_rate - 50.0).abs() < f64::EPSILON);
}

#[test]
fn test_percentile_with_known_durations() {
    let (conn, qm) = setup();
    {
        let c = conn.lock().unwrap();
        for i in 0..3 {
            let id = format!("00000000-0000-0000-0000-{:012}", i + 1);
            let created = 1000 + (i * 100);
            let completed = created + (i as i64 + 1) * 100;
            insert_job(&c, &id, "HP", "COMPLETED", created, Some(completed));
        }
    }

    let collector = MetricsCollector::new(conn, qm);
    let snapshot = collector.collect_metrics().unwrap();

    let durations: Vec<f64> = vec![100.0, 200.0, 300.0];
    let mut sorted = durations.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    assert!((snapshot.performance_metrics.avg_job_duration_secs - 200.0).abs() < f64::EPSILON);
    assert!((snapshot.performance_metrics.p50_job_duration_secs - 200.0).abs() < f64::EPSILON);
    assert!((snapshot.performance_metrics.p95_job_duration_secs - 290.0).abs() < 1.0);
    assert!((snapshot.performance_metrics.p99_job_duration_secs - 298.0).abs() < 1.0);
}

#[test]
fn test_full_lifecycle_metrics() {
    let (conn, qm) = setup();
    {
        let c = conn.lock().unwrap();
        let job_id = "00000000-0000-0000-0000-000000000001";
        insert_job(&c, job_id, "HP", "COMPLETED", 1000, Some(2000));

        insert_event(&c, job_id, 1, "PrintJobCreated", 1000);
        insert_event(&c, job_id, 2, "PrintJobQueued", 1010);
        insert_event(&c, job_id, 3, "PrintJobDownloaded", 1050);
        insert_event(&c, job_id, 4, "PrintJobSubmitted", 1070);
        insert_event(&c, job_id, 5, "PrintJobPrinting", 1080);
        insert_event(&c, job_id, 6, "PrintJobCompleted", 2000);
    }

    let collector = MetricsCollector::new(conn, qm);
    let snapshot = collector.collect_metrics().unwrap();

    assert_eq!(snapshot.job_metrics.total_jobs, 1);
    assert_eq!(snapshot.job_metrics.completed, 1);
    assert!((snapshot.job_metrics.success_rate - 100.0).abs() < f64::EPSILON);

    assert!((snapshot.performance_metrics.avg_job_duration_secs - 1000.0).abs() < f64::EPSILON);
    assert!((snapshot.performance_metrics.avg_download_time_secs - 40.0).abs() < f64::EPSILON);
    assert!((snapshot.performance_metrics.avg_render_time_secs - 20.0).abs() < f64::EPSILON);
    assert!((snapshot.performance_metrics.avg_print_time_secs - 920.0).abs() < f64::EPSILON);
}

#[test]
fn test_printer_metrics_grouping() {
    let (conn, qm) = setup();
    {
        let c = conn.lock().unwrap();
        insert_job(&c, "00000000-0000-0000-0000-000000000001", "PrinterA", "COMPLETED", 1000, Some(2000));
        insert_job(&c, "00000000-0000-0000-0000-000000000002", "PrinterA", "COMPLETED", 1000, Some(2000));
        insert_job(&c, "00000000-0000-0000-0000-000000000003", "PrinterA", "FAILED", 1000, None);
        insert_job(&c, "00000000-0000-0000-0000-000000000004", "PrinterB", "COMPLETED", 1000, Some(2000));
        insert_job(&c, "00000000-0000-0000-0000-000000000005", "PrinterB", "COMPLETED", 1000, Some(2000));
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
