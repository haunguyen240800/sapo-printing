use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::print_job::aggregate::PrintJob;
use crate::domain::print_job::errors::DomainError;
use crate::domain::print_job::repository::PrintJobRepository;
use crate::domain::print_job::value_objects::{JobId, PrintStatus};

/// SQLite implementation of `PrintJobRepository`.
///
/// Persists `PrintJob` aggregates to the `print_jobs` table.
/// Uses prepared statements for all queries. Locks mutex before each operation.
pub struct SqlitePrintJobRepository {
    conn: Arc<Mutex<Connection>>,
}

impl SqlitePrintJobRepository {
    /// Create a new repository with a shared database connection.
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }
}

impl PrintJobRepository for SqlitePrintJobRepository {
    fn save(&self, job: &PrintJob) -> Result<(), DomainError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let completed_at = completed_at_for_status(job.status(), now);

        let rows = conn
            .execute(
                "INSERT INTO print_jobs (id, printer_name, document_url, status, retry_count, created_at, updated_at, completed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    job.id().to_string(),
                    job.printer_name(),
                    job.pdf_url(),
                    status_to_string(job.status()),
                    job.retry_count() as i64,
                    now,
                    now,
                    completed_at,
                ],
            )
            .map_err(|e| {
                // rusqlite SQLITE_CONSTRAINT errors map to DatabaseError with code ConstraintViolation
                if matches!(
                    &e,
                    rusqlite::Error::SqliteFailure(
                        rusqlite::ffi::Error {
                            code: rusqlite::ErrorCode::ConstraintViolation,
                            ..
                        },
                        _
                    )
                ) {
                    DomainError::RepositoryError {
                        reason: format!("Job '{}' already exists", job.id()),
                    }
                } else {
                    DomainError::RepositoryError {
                        reason: format!("Failed to save print job: {}", e),
                    }
                }
            })?;

        if rows == 0 {
            return Err(DomainError::RepositoryError {
                reason: "Failed to save print job: no rows inserted".into(),
            });
        }

        Ok(())
    }

    fn update(&self, job: &PrintJob) -> Result<(), DomainError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let completed_at = completed_at_for_status(job.status(), now);

        let rows = conn
            .execute(
                "UPDATE print_jobs SET status = ?1, retry_count = ?2, updated_at = ?3, completed_at = ?4 WHERE id = ?5",
                rusqlite::params![
                    status_to_string(job.status()),
                    job.retry_count() as i64,
                    now,
                    completed_at,
                    job.id().to_string(),
                ],
            )
            .map_err(|e| DomainError::RepositoryError {
                reason: format!("Failed to update print job: {}", e),
            })?;

        if rows == 0 {
            return Err(DomainError::RepositoryError {
                reason: format!("Job '{}' not found", job.id()),
            });
        }

        Ok(())
    }

    fn find_by_id(&self, id: &JobId) -> Result<Option<PrintJob>, DomainError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());

        let mut stmt = conn
            .prepare(
                "SELECT id, printer_name, document_url, status, retry_count
                 FROM print_jobs WHERE id = ?1",
            )
            .map_err(|e| DomainError::RepositoryError {
                reason: format!("Failed to prepare query: {}", e),
            })?;

        let result = stmt.query_row([id.to_string()], |row| row_to_print_job(row));

        match result {
            Ok(job) => Ok(Some(job)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(DomainError::RepositoryError {
                reason: format!("Failed to query print job: {}", e),
            }),
        }
    }

    fn find_by_status(&self, status: &PrintStatus) -> Result<Vec<PrintJob>, DomainError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());

        let mut stmt = conn
            .prepare(
                "SELECT id, printer_name, document_url, status, retry_count
                 FROM print_jobs WHERE status = ?1",
            )
            .map_err(|e| DomainError::RepositoryError {
                reason: format!("Failed to prepare query: {}", e),
            })?;

        let job_iter = stmt
            .query_map([status_to_string(status)], |row| row_to_print_job(row))
            .map_err(|e| DomainError::RepositoryError {
                reason: format!("Failed to query print jobs: {}", e),
            })?;

        let mut jobs = Vec::new();
        for job_result in job_iter {
            jobs.push(job_result.map_err(|e| DomainError::RepositoryError {
                reason: format!("Failed to read print job row: {}", e),
            })?);
        }

        Ok(jobs)
    }

    fn find_all(&self) -> Result<Vec<PrintJob>, DomainError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());

        let mut stmt = conn
            .prepare(
                "SELECT id, printer_name, document_url, status, retry_count
                 FROM print_jobs",
            )
            .map_err(|e| DomainError::RepositoryError {
                reason: format!("Failed to prepare query: {}", e),
            })?;

        let job_iter = stmt
            .query_map([], |row| row_to_print_job(row))
            .map_err(|e| DomainError::RepositoryError {
                reason: format!("Failed to query print jobs: {}", e),
            })?;

        let mut jobs = Vec::new();
        for job_result in job_iter {
            jobs.push(job_result.map_err(|e| DomainError::RepositoryError {
                reason: format!("Failed to read print job row: {}", e),
            })?);
        }

        Ok(jobs)
    }
}

/// Helper: convert a row to a PrintJob via reconstruct().
fn row_to_print_job(row: &rusqlite::Row<'_>) -> Result<PrintJob, rusqlite::Error> {
    let id_str: String = row.get(0)?;
    let printer_name: String = row.get(1)?;
    let document_url: String = row.get(2)?;
    let status_str: String = row.get(3)?;
    let retry_count: i64 = row.get(4)?;

    let id: JobId = id_str.parse().map_err(|e: uuid::Error| {
        rusqlite::Error::InvalidColumnType(
            0,
            format!("Invalid UUID: {}", e),
            rusqlite::types::Type::Text,
        )
    })?;

    let status = status_from_string(&status_str).map_err(|e| {
        rusqlite::Error::InvalidColumnType(
            3,
            format!("Invalid status: {:?}", e),
            rusqlite::types::Type::Text,
        )
    })?;

    Ok(PrintJob::reconstruct(
        id,
        status,
        retry_count as u32,
        document_url,
        printer_name,
    ))
}

/// Helper: PrintStatus → database TEXT.
fn status_to_string(s: &PrintStatus) -> String {
    format!("{:?}", s)
}

/// Helper: database TEXT → PrintStatus.
fn status_from_string(s: &str) -> Result<PrintStatus, DomainError> {
    match s {
        "Pending" => Ok(PrintStatus::Pending),
        "Queued" => Ok(PrintStatus::Queued),
        "Downloaded" => Ok(PrintStatus::Downloaded),
        "SubmittedToQueue" => Ok(PrintStatus::SubmittedToQueue),
        "Printing" => Ok(PrintStatus::Printing),
        "Completed" => Ok(PrintStatus::Completed),
        "Failed" => Ok(PrintStatus::Failed),
        "Cancelled" => Ok(PrintStatus::Cancelled),
        _ => Err(DomainError::InvalidStatus {
            status: s.to_string(),
        }),
    }
}

/// Helper: determine completed_at based on status (uses the same timestamp as created_at/updated_at).
fn completed_at_for_status(status: &PrintStatus, now: i64) -> Option<i64> {
    match status {
        PrintStatus::Completed | PrintStatus::Failed | PrintStatus::Cancelled => Some(now),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database::migrations::run_migrations;

    fn setup_test_db() -> Arc<Mutex<Connection>> {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();
        Arc::new(Mutex::new(conn))
    }

    fn make_test_job() -> PrintJob {
        PrintJob::new(
            "https://s3.example.com/doc.pdf".to_string(),
            "HP_LaserJet".to_string(),
        )
    }

    #[test]
    fn test_save_and_find_by_id() {
        let conn = setup_test_db();
        let repo = SqlitePrintJobRepository::new(conn);

        let job = make_test_job();
        let job_id = job.id().clone();
        repo.save(&job).unwrap();

        let found = repo.find_by_id(&job_id).unwrap();
        assert!(found.is_some());
        let found = found.unwrap();
        assert_eq!(found.id(), &job_id);
        assert_eq!(found.printer_name(), "HP_LaserJet");
        assert_eq!(found.pdf_url(), "https://s3.example.com/doc.pdf");
        assert_eq!(*found.status(), PrintStatus::Pending);
        assert_eq!(found.retry_count(), 0);

        // Non-existent ID → None
        let missing = JobId::new();
        let result = repo.find_by_id(&missing).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_update_job() {
        let conn = setup_test_db();
        let repo = SqlitePrintJobRepository::new(conn);

        let mut job = make_test_job();
        let job_id = job.id().clone();
        repo.save(&job).unwrap();

        // Transition to Queued and update
        job.queue().unwrap();
        repo.update(&job).unwrap();

        let found = repo.find_by_id(&job_id).unwrap().unwrap();
        assert_eq!(*found.status(), PrintStatus::Queued);
    }

    #[test]
    fn test_find_by_status() {
        let conn = setup_test_db();
        let repo = SqlitePrintJobRepository::new(conn);

        // 2 PENDING jobs
        let job1 = make_test_job();
        let job2 = make_test_job();
        repo.save(&job1).unwrap();
        repo.save(&job2).unwrap();

        // 1 QUEUED job
        let mut job3 = make_test_job();
        job3.queue().unwrap();
        repo.save(&job3).unwrap();

        let pending = repo.find_by_status(&PrintStatus::Pending).unwrap();
        assert_eq!(pending.len(), 2);

        let queued = repo.find_by_status(&PrintStatus::Queued).unwrap();
        assert_eq!(queued.len(), 1);

        let printing = repo.find_by_status(&PrintStatus::Printing).unwrap();
        assert!(printing.is_empty());
    }

    #[test]
    fn test_find_all() {
        let conn = setup_test_db();
        let repo = SqlitePrintJobRepository::new(conn);

        let job1 = make_test_job();
        let job2 = make_test_job();
        repo.save(&job1).unwrap();
        repo.save(&job2).unwrap();

        let all = repo.find_all().unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_save_duplicate_job_fails() {
        let conn = setup_test_db();
        let repo = SqlitePrintJobRepository::new(conn);

        let job = make_test_job();
        repo.save(&job).unwrap();

        // Second save of same job ID should fail with constraint error
        let result = repo.save(&job);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            format!("{}", err).contains("already exists"),
            "Expected 'already exists' error, got: {}",
            err
        );
    }

    #[test]
    fn test_update_nonexistent_job_fails() {
        let conn = setup_test_db();
        let repo = SqlitePrintJobRepository::new(conn);

        let job = make_test_job();
        let result = repo.update(&job);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            format!("{}", err).contains("not found"),
            "Expected 'not found' error, got: {}",
            err
        );
    }

    #[test]
    fn test_find_by_invalid_status_returns_error() {
        let conn = setup_test_db();
        let job = make_test_job();
        // Insert a row with an invalid status string directly
        {
            let c = conn.lock().unwrap();
            c.execute(
                "INSERT INTO print_jobs (id, printer_name, document_url, status, retry_count, created_at, updated_at, completed_at)
                 VALUES (?1, ?2, ?3, 'UnknownStatus', ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    job.id().to_string(),
                    job.printer_name(),
                    job.pdf_url(),
                    0i64,
                    0i64,
                    0i64,
                    None::<i64>,
                ],
            )
            .unwrap();
        }

        let repo = SqlitePrintJobRepository::new(conn);
        let result = repo.find_by_id(job.id());
        assert!(result.is_err());
    }
}
