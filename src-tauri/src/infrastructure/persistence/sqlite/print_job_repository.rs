use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::models::PrintJob;
use crate::domain::models::DomainError;
use crate::domain::repository::PrintJobRepository;
use crate::domain::models::{JobId, PrintStatus};

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
        tracing::info!(
            target = "sapo_printer::repository::print_job",
            operation = "save",
            job_id = %job.id(),
            "SqlitePrintJobRepository::save() - STARTING"
        );

        let lock_start = std::time::Instant::now();
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let lock_duration = lock_start.elapsed();

        tracing::info!(
            target = "sapo_printer::repository::print_job",
            operation = "save",
            job_id = %job.id(),
            lock_wait_ms = lock_duration.as_millis(),
            "SqlitePrintJobRepository::save() - got database lock"
        );

        if lock_duration.as_secs() > 5 {
            tracing::warn!(
                target = "sapo_printer::repository::print_job",
                operation = "save",
                job_id = %job.id(),
                lock_wait_secs = lock_duration.as_secs(),
                "SqlitePrintJobRepository::save() - SLOW LOCK (waited >5s)"
            );
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let completed_at = completed_at_for_status(job.status(), now);

        tracing::debug!(
            target = "sapo_printer::repository::print_job",
            operation = "save",
            job_id = %job.id(),
            status = ?job.status(),
            "INSERT INTO print_jobs"
        );

        tracing::info!(
            target = "sapo_printer::repository::print_job",
            operation = "save",
            job_id = %job.id(),
            "SqlitePrintJobRepository::save() - executing INSERT"
        );

        let rows = conn
            .execute(
                "INSERT INTO print_jobs (id, printer_name, document_url, status, retry_count, created_at, updated_at, completed_at, error_message, output_path)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                rusqlite::params![
                    job.id().to_string(),
                    job.printer_name(),
                    job.pdf_url(),
                    status_to_string(job.status()),
                    job.retry_count() as i64,
                    now,
                    now,
                    completed_at,
                    job.error_message(),
                    job.output_path(),
                ],
            )
            .map_err(|e| {
                tracing::error!(
                    target = "sapo_printer::repository::print_job",
                    operation = "save",
                    job_id = %job.id(),
                    error = %e,
                    "Failed to save print job"
                );
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

        tracing::info!(
            target = "sapo_printer::repository::print_job",
            operation = "save",
            job_id = %job.id(),
            rows = rows,
            "SqlitePrintJobRepository::save() - INSERT SUCCESS"
        );

        Ok(())
    }

    fn update(&self, job: &PrintJob) -> Result<(), DomainError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let completed_at = completed_at_for_status(job.status(), now);

        tracing::debug!(
            target = "sapo_printer::repository::print_job",
            operation = "update",
            job_id = %job.id(),
            status = ?job.status(),
            "UPDATE print_jobs"
        );

        let rows = conn
            .execute(
                "UPDATE print_jobs SET status = ?1, retry_count = ?2, updated_at = ?3, completed_at = ?4, error_message = ?5 WHERE id = ?6",
                rusqlite::params![
                    status_to_string(job.status()),
                    job.retry_count() as i64,
                    now,
                    completed_at,
                    job.error_message(),
                    job.id().to_string(),
                ],
            )
            .map_err(|e| {
                tracing::error!(
                    target = "sapo_printer::repository::print_job",
                    operation = "update",
                    job_id = %job.id(),
                    error = %e,
                    "Failed to update print job"
                );
                DomainError::RepositoryError {
                    reason: format!("Failed to update print job: {}", e),
                }
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

        tracing::debug!(
            target = "sapo_printer::repository::print_job",
            operation = "find_by_id",
            job_id = %id,
            "SELECT FROM print_jobs"
        );

        let mut stmt = conn
            .prepare(
                "SELECT id, printer_name, document_url, status, retry_count, created_at, completed_at, error_message, output_path
                 FROM print_jobs WHERE id = ?1",
            )
            .map_err(|e| {
                tracing::error!(
                    target = "sapo_printer::repository::print_job",
                    operation = "find_by_id",
                    job_id = %id,
                    error = %e,
                    "Failed to prepare query"
                );
                DomainError::RepositoryError {
                    reason: format!("Failed to prepare query: {}", e),
                }
            })?;

        let result = stmt.query_row([id.to_string()], |row| row_to_print_job(row));

        match result {
            Ok(job) => Ok(Some(job)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => {
                tracing::error!(
                    target = "sapo_printer::repository::print_job",
                    operation = "find_by_id",
                    job_id = %id,
                    error = %e,
                    "Failed to query print job"
                );
                Err(DomainError::RepositoryError {
                    reason: format!("Failed to query print job: {}", e),
                })
            }
        }
    }

    fn find_by_status(&self, status: &PrintStatus) -> Result<Vec<PrintJob>, DomainError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());

        tracing::debug!(
            target = "sapo_printer::repository::print_job",
            operation = "find_by_status",
            status = ?status,
            "SELECT FROM print_jobs WHERE status"
        );

        let mut stmt = conn
            .prepare(
                "SELECT id, printer_name, document_url, status, retry_count, created_at, completed_at, error_message, output_path
                 FROM print_jobs WHERE status = ?1",
            )
            .map_err(|e| {
                tracing::error!(
                    target = "sapo_printer::repository::print_job",
                    operation = "find_by_status",
                    error = %e,
                    "Failed to prepare query"
                );
                DomainError::RepositoryError {
                    reason: format!("Failed to prepare query: {}", e),
                }
            })?;

        let job_iter = stmt
            .query_map([status_to_string(status)], |row| row_to_print_job(row))
            .map_err(|e| {
                tracing::error!(
                    target = "sapo_printer::repository::print_job",
                    operation = "find_by_status",
                    error = %e,
                    "Failed to query print jobs"
                );
                DomainError::RepositoryError {
                    reason: format!("Failed to query print jobs: {}", e),
                }
            })?;

        let mut jobs = Vec::new();
        for job_result in job_iter {
            jobs.push(job_result.map_err(|e| {
                tracing::error!(
                    target = "sapo_printer::repository::print_job",
                    operation = "find_by_status",
                    error = %e,
                    "Failed to read print job row"
                );
                DomainError::RepositoryError {
                    reason: format!("Failed to read print job row: {}", e),
                }
            })?);
        }

        Ok(jobs)
    }

    fn find_all(&self) -> Result<Vec<PrintJob>, DomainError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());

        tracing::debug!(
            target = "sapo_printer::repository::print_job",
            operation = "find_all",
            "SELECT FROM print_jobs"
        );

        let mut stmt = conn
            .prepare(
                "SELECT id, printer_name, document_url, status, retry_count, created_at, completed_at, error_message, output_path
                 FROM print_jobs",
            )
            .map_err(|e| {
                tracing::error!(
                    target = "sapo_printer::repository::print_job",
                    operation = "find_all",
                    error = %e,
                    "Failed to prepare query"
                );
                DomainError::RepositoryError {
                    reason: format!("Failed to prepare query: {}", e),
                }
            })?;

        let job_iter = stmt
            .query_map([], |row| row_to_print_job(row))
            .map_err(|e| {
                tracing::error!(
                    target = "sapo_printer::repository::print_job",
                    operation = "find_all",
                    error = %e,
                    "Failed to query print jobs"
                );
                DomainError::RepositoryError {
                    reason: format!("Failed to query print jobs: {}", e),
                }
            })?;

        let mut jobs = Vec::new();
        for job_result in job_iter {
            jobs.push(job_result.map_err(|e| {
                tracing::error!(
                    target = "sapo_printer::repository::print_job",
                    operation = "find_all",
                    error = %e,
                    "Failed to read print job row"
                );
                DomainError::RepositoryError {
                    reason: format!("Failed to read print job row: {}", e),
                }
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
    let created_at: i64 = row.get(5)?;
    let completed_at: Option<i64> = row.get(6)?;
    let error_message: Option<String> = row.get(7)?;
    let output_path: Option<String> = row.get(8)?;

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
        created_at,
        completed_at,
        error_message,
        output_path,
        crate::domain::models::PrintJobSettings::default(),
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


