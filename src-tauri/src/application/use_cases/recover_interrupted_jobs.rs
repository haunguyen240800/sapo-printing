use std::sync::Arc;

use crate::application::errors::Error;
use crate::application::ports::EventStore;
use crate::application::ports::event_bus::EventBus;
use crate::domain::print_job::{PrintJob, PrintJobRepository, PrintStatus};

/// Dọn dẹp các print job còn dang dở khi ứng dụng khởi động.
///
/// Sau khi app bị tắt đột ngột (crash / kill / reboot máy), DB có thể còn job ở
/// trạng thái non-terminal. Nếu để nguyên, `QueueWorker` sẽ `pop()` lại các job
/// `QUEUED` cũ và in lại phiếu đã lỗi thời; còn job đang `PROCESSING/PRINTING`
/// (bị gián đoạn giữa chừng) sẽ kẹt vĩnh viễn vì `pop()` không bao giờ chọn lại
/// các trạng thái đó.
///
/// Use case này đưa mọi job non-terminal về trạng thái terminal, với ngữ nghĩa
/// phân biệt để audit trail rõ ràng:
/// - `PROCESSING/DOWNLOADED/SUBMITTED_TO_QUEUE/PRINTING` (đang in dở) → `FAILED`.
/// - `PENDING/QUEUED` (chưa từng chạy pipeline) → `CANCELLED`.
///
/// PHẢI được gọi TRƯỚC `QueueWorker::start()` để không có cửa sổ race mà worker
/// kịp `pop()` và in một job trong lúc đang dọn.
pub struct RecoverInterruptedJobsUseCase {
    job_repo: Arc<dyn PrintJobRepository>,
    event_store: Arc<dyn EventStore>,
    event_bus: Arc<dyn EventBus>,
}

const NON_TERMINAL_STATUSES: [PrintStatus; 6] = [
    PrintStatus::Pending,
    PrintStatus::Queued,
    PrintStatus::Processing,
    PrintStatus::Downloaded,
    PrintStatus::SubmittedToQueue,
    PrintStatus::Printing,
];

impl RecoverInterruptedJobsUseCase {
    pub fn new(
        job_repo: Arc<dyn PrintJobRepository>,
        event_store: Arc<dyn EventStore>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self {
            job_repo,
            event_store,
            event_bus,
        }
    }

    /// Trả về số job đã được đưa về terminal trong lần dọn này.
    pub fn execute(&self) -> Result<usize, Error> {
        tracing::info!(
            target = "sapo_printer::application::use_case::recover_interrupted_jobs",
            "RecoverInterruptedJobsUseCase: starting startup reconciliation"
        );

        let mut recovered = 0usize;
        for status in NON_TERMINAL_STATUSES {
            let jobs = self
                .job_repo
                .find_by_status(&status)
                .map_err(|e| Error::RepositoryError(e.to_string()))?;

            for mut job in jobs {
                if self.reconcile_job(&mut job)? {
                    recovered += 1;
                }
            }
        }

        tracing::info!(
            target = "sapo_printer::application::use_case::recover_interrupted_jobs",
            recovered = recovered,
            "RecoverInterruptedJobsUseCase: completed"
        );

        Ok(recovered)
    }

    /// Đưa một job non-terminal về terminal. Trả về `true` nếu có transition xảy ra.
    fn reconcile_job(&self, job: &mut PrintJob) -> Result<bool, Error> {
        let transition = match job.status() {
            // Chưa từng chạy pipeline → hủy như một phiếu bị bỏ qua.
            PrintStatus::Pending | PrintStatus::Queued => job.cancel(),
            // Đang in dở khi bị tắt → đánh dấu thất bại do gián đoạn.
            PrintStatus::Processing
            | PrintStatus::Downloaded
            | PrintStatus::SubmittedToQueue
            | PrintStatus::Printing => job.fail(
                "Ứng dụng đã khởi động lại khi phiếu đang được xử lý; job bị hủy để tránh in lại."
                    .to_string(),
                "APP_RESTART_INTERRUPTED".to_string(),
            ),
            // find_by_status chỉ trả về các trạng thái non-terminal ở trên.
            _ => return Ok(false),
        };

        if let Err(e) = transition {
            tracing::warn!(
                target = "sapo_printer::application::use_case::recover_interrupted_jobs",
                job_id = %job.id(),
                error = %e,
                "Skip job that cannot be reconciled"
            );
            return Ok(false);
        }

        let events = job.drain_events();

        self.event_store
            .save_all(job.id().to_string().as_str(), &events)
            .map_err(|e| Error::RepositoryError(e.to_string()))?;

        self.job_repo
            .update(job)
            .map_err(|e| Error::RepositoryError(e.to_string()))?;

        for event in &events {
            if let Err(e) = self
                .event_bus
                .publish(event.event_name(), &event.serialize_payload())
            {
                tracing::warn!(
                    target = "sapo_printer::application::use_case::recover_interrupted_jobs",
                    event_type = %event.event_name(),
                    error = %e,
                    "Event bus publish failed (non-fatal)"
                );
            }
        }

        tracing::info!(
            target = "sapo_printer::application::use_case::recover_interrupted_jobs",
            job_id = %job.id(),
            new_status = ?job.status(),
            "Reconciled interrupted job on startup"
        );

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::print_job::PrintJobId;
    use crate::infrastructure::configs::db::{DbPool, run_migrations};
    use crate::infrastructure::eventbus::InMemoryEventBus;
    use crate::infrastructure::persistence::{
        EventRepository, PrintJobRepository as SqlitePrintJobRepository,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    fn setup() -> (DbPool, RecoverInterruptedJobsUseCase, std::path::PathBuf) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let thread_id = std::thread::current().id();
        let path = std::env::temp_dir().join(format!("sapo_recover_test_{nanos}_{thread_id:?}.db"));
        let pool = DbPool::new(path.to_str().unwrap()).unwrap();
        {
            let mut conn = pool.get().unwrap();
            run_migrations(&mut *conn).unwrap();
        }
        let job_repo: Arc<dyn PrintJobRepository> =
            Arc::new(SqlitePrintJobRepository::new(pool.clone()));
        let event_store: Arc<dyn EventStore> = Arc::new(EventRepository::new(pool.clone()));
        let event_bus: Arc<dyn EventBus> = Arc::new(InMemoryEventBus::new());
        let uc = RecoverInterruptedJobsUseCase::new(job_repo, event_store, event_bus);
        (pool, uc, path)
    }

    fn cleanup(path: &std::path::PathBuf) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }

    fn insert_job(pool: &DbPool, status: &str) -> PrintJobId {
        let id = PrintJobId::new();
        let conn = pool.get().unwrap();
        let now = 1_700_000_000i64;
        conn.execute(
            "INSERT INTO print_jobs (id, slip_id, printer_name, document_url, status, retry_count, created_at, updated_at)
             VALUES (?1, 'slip-1', 'HP', 'https://s3.example.com/doc.pdf', ?2, 0, ?3, ?3)",
            rusqlite::params![id.to_string(), status, now],
        )
        .unwrap();
        id
    }

    fn status_of(pool: &DbPool, id: &PrintJobId) -> String {
        let conn = pool.get().unwrap();
        conn.query_row(
            "SELECT status FROM print_jobs WHERE id = ?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .unwrap()
    }

    #[test]
    fn test_pending_and_queued_are_cancelled() {
        let (pool, uc, path) = setup();
        let pending = insert_job(&pool, "PENDING");
        let queued = insert_job(&pool, "QUEUED");

        let recovered = uc.execute().unwrap();
        assert_eq!(recovered, 2);
        assert_eq!(status_of(&pool, &pending), "CANCELLED");
        assert_eq!(status_of(&pool, &queued), "CANCELLED");
        drop(pool);
        cleanup(&path);
    }

    #[test]
    fn test_in_flight_states_are_failed() {
        let (pool, uc, path) = setup();
        let processing = insert_job(&pool, "PROCESSING");
        let downloaded = insert_job(&pool, "DOWNLOADED");
        let submitted = insert_job(&pool, "SUBMITTED_TO_QUEUE");
        let printing = insert_job(&pool, "PRINTING");

        let recovered = uc.execute().unwrap();
        assert_eq!(recovered, 4);
        assert_eq!(status_of(&pool, &processing), "FAILED");
        assert_eq!(status_of(&pool, &downloaded), "FAILED");
        assert_eq!(status_of(&pool, &submitted), "FAILED");
        assert_eq!(status_of(&pool, &printing), "FAILED");
        drop(pool);
        cleanup(&path);
    }

    #[test]
    fn test_terminal_jobs_are_untouched() {
        let (pool, uc, path) = setup();
        let completed = insert_job(&pool, "COMPLETED");
        let failed = insert_job(&pool, "FAILED");
        let cancelled = insert_job(&pool, "CANCELLED");

        let recovered = uc.execute().unwrap();
        assert_eq!(recovered, 0);
        assert_eq!(status_of(&pool, &completed), "COMPLETED");
        assert_eq!(status_of(&pool, &failed), "FAILED");
        assert_eq!(status_of(&pool, &cancelled), "CANCELLED");
        drop(pool);
        cleanup(&path);
    }

    #[test]
    fn test_recovered_job_is_not_popped_afterwards() {
        // Sau reconciliation, một job QUEUED cũ đã thành CANCELLED nên worker
        // (pop chỉ chọn QUEUED) không được lấy lại để in.
        use crate::application::ports::QueuePort;
        use crate::infrastructure::persistence::JobQueueBroker;

        let (pool, uc, path) = setup();
        insert_job(&pool, "QUEUED");

        uc.execute().unwrap();

        let broker = JobQueueBroker::new(pool.clone());
        assert!(broker.pop().unwrap().is_none());

        drop(pool);
        cleanup(&path);
    }

    #[test]
    fn test_events_persisted_for_reconciled_jobs() {
        let (pool, uc, path) = setup();
        let job = insert_job(&pool, "PRINTING");

        uc.execute().unwrap();

        let conn = pool.get().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM events WHERE aggregate_id = ?1",
                [job.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert!(count >= 1, "phải ghi ít nhất một event khi reconcile");
        drop(conn);
        drop(pool);
        cleanup(&path);
    }
}
