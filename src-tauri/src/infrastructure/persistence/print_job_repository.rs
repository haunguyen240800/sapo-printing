use std::time::{SystemTime, UNIX_EPOCH};

use crate::domain::print_job::{
    PrintJob, PrintJobError, PrintJobId, PrintJobRepository as PrintJobRepositoryPort, PrintStatus, PrinterId,
};
use crate::infrastructure::configs::db::DbPool;

pub struct PrintJobRepository {
    pool: DbPool,
}

impl PrintJobRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    fn acquire(&self) -> Result<crate::infrastructure::configs::db::SqliteConn, PrintJobError> {
        self.pool.get().map_err(|e| PrintJobError::RepositoryError {
            reason: format!("Failed to acquire DB connection: {}", e),
        })
    }
}

impl PrintJobRepositoryPort for PrintJobRepository {
    fn save(&self, job: &PrintJob) -> Result<(), PrintJobError> {
        let acquire_start = std::time::Instant::now();
        let conn = self.acquire()?;
        let acquire_duration = acquire_start.elapsed();

        if acquire_duration.as_secs() > 5 {
            tracing::warn!(
                target = "sapo_printer::repository::print_job",
                operation = "save",
                job_id = %job.id(),
                pool_wait_secs = acquire_duration.as_secs(),
                "PrintJobRepository::save() - SLOW POOL ACQUIRE (waited >5s)"
            );
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let completed_at = completed_at_for_status(job.status(), now);

        let settings_json =
            serde_json::to_string(&job.settings).map_err(|e| PrintJobError::RepositoryError {
                reason: format!("Failed to serialize job settings: {}", e),
            })?;

        tracing::trace!(
            target = "sapo_printer::repository::print_job",
            operation = "save",
            job_id = %job.id(),
            status = ?job.status(),
            pool_wait_ms = acquire_duration.as_millis(),
            "INSERT INTO print_jobs"
        );

        let rows = conn
            .execute(
                "INSERT INTO print_jobs (id, printer_name, document_url, status, retry_count, created_at, updated_at, completed_at, error_message, output_path, settings_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![
                    job.id().to_string(),
                    job.printer_id().as_str(),
                    job.pdf_url(),
                    job.status().to_db_string(),
                    job.retry_count() as i64,
                    now,
                    now,
                    completed_at,
                    job.error_message(),
                    job.output_path(),
                    settings_json,
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
                    PrintJobError::RepositoryError {
                        reason: format!("Job '{}' already exists", job.id()),
                    }
                } else {
                    PrintJobError::RepositoryError {
                        reason: format!("Failed to save print job: {}", e),
                    }
                }
            })?;

        if rows == 0 {
            return Err(PrintJobError::RepositoryError {
                reason: "Failed to save print job: no rows inserted".into(),
            });
        }

        tracing::trace!(
            target = "sapo_printer::repository::print_job",
            operation = "save",
            job_id = %job.id(),
            rows = rows,
            "INSERT print_jobs OK"
        );

        Ok(())
    }

    fn update(&self, job: &PrintJob) -> Result<(), PrintJobError> {
        let conn = self.acquire()?;

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
                    job.status().to_db_string(),
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
                PrintJobError::RepositoryError {
                    reason: format!("Failed to update print job: {}", e),
                }
            })?;

        if rows == 0 {
            return Err(PrintJobError::RepositoryError {
                reason: format!("Job '{}' not found", job.id()),
            });
        }

        Ok(())
    }

    fn find_by_id(&self, id: &PrintJobId) -> Result<Option<PrintJob>, PrintJobError> {
        let conn = self.acquire()?;

        tracing::debug!(
            target = "sapo_printer::repository::print_job",
            operation = "find_by_id",
            job_id = %id,
            "SELECT FROM print_jobs"
        );

        let mut stmt = conn
            .prepare(
                "SELECT id, printer_name, document_url, status, retry_count, created_at, completed_at, error_message, output_path, settings_json
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
                PrintJobError::RepositoryError {
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
                Err(PrintJobError::RepositoryError {
                    reason: format!("Failed to query print job: {}", e),
                })
            }
        }
    }

    fn find_by_status(&self, status: &PrintStatus) -> Result<Vec<PrintJob>, PrintJobError> {
        let conn = self.acquire()?;

        tracing::debug!(
            target = "sapo_printer::repository::print_job",
            operation = "find_by_status",
            status = ?status,
            "SELECT FROM print_jobs WHERE status"
        );

        let mut stmt = conn
            .prepare(
                "SELECT id, printer_name, document_url, status, retry_count, created_at, completed_at, error_message, output_path, settings_json
                 FROM print_jobs WHERE status = ?1",
            )
            .map_err(|e| {
                tracing::error!(
                    target = "sapo_printer::repository::print_job",
                    operation = "find_by_status",
                    error = %e,
                    "Failed to prepare query"
                );
                PrintJobError::RepositoryError {
                    reason: format!("Failed to prepare query: {}", e),
                }
            })?;

        let job_iter = stmt
            .query_map([status.to_db_string()], |row| row_to_print_job(row))
            .map_err(|e| {
                tracing::error!(
                    target = "sapo_printer::repository::print_job",
                    operation = "find_by_status",
                    error = %e,
                    "Failed to query print jobs"
                );
                PrintJobError::RepositoryError {
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
                PrintJobError::RepositoryError {
                    reason: format!("Failed to read print job row: {}", e),
                }
            })?);
        }

        Ok(jobs)
    }

    fn find_all(&self) -> Result<Vec<PrintJob>, PrintJobError> {
        let conn = self.acquire()?;

        tracing::debug!(
            target = "sapo_printer::repository::print_job",
            operation = "find_all",
            "SELECT FROM print_jobs"
        );

        let mut stmt = conn
            .prepare(
                "SELECT id, printer_name, document_url, status, retry_count, created_at, completed_at, error_message, output_path, settings_json
                 FROM print_jobs",
            )
            .map_err(|e| {
                tracing::error!(
                    target = "sapo_printer::repository::print_job",
                    operation = "find_all",
                    error = %e,
                    "Failed to prepare query"
                );
                PrintJobError::RepositoryError {
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
                PrintJobError::RepositoryError {
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
                PrintJobError::RepositoryError {
                    reason: format!("Failed to read print job row: {}", e),
                }
            })?);
        }

        Ok(jobs)
    }
}

fn row_to_print_job(row: &rusqlite::Row<'_>) -> Result<PrintJob, rusqlite::Error> {
    let id_str: String = row.get(0)?;
    let printer_id_raw: String = row.get(1)?;
    let document_url: String = row.get(2)?;
    let status_str: String = row.get(3)?;
    let retry_count: i64 = row.get(4)?;
    let created_at: i64 = row.get(5)?;
    let completed_at: Option<i64> = row.get(6)?;
    let error_message: Option<String> = row.get(7)?;
    let output_path: Option<String> = row.get(8)?;
    let settings_json: Option<String> = row.get(9)?;

    let id: PrintJobId = id_str.parse().map_err(|e: uuid::Error| {
        rusqlite::Error::InvalidColumnType(
            0,
            format!("Invalid UUID: {}", e),
            rusqlite::types::Type::Text,
        )
    })?;

    let status = PrintStatus::from_db_string(&status_str).map_err(|invalid| {
        rusqlite::Error::InvalidColumnType(
            3,
            format!("Invalid status: {}", invalid),
            rusqlite::types::Type::Text,
        )
    })?;

    let settings = settings_json
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    Ok(PrintJob::reconstruct(
        id,
        status,
        retry_count as u32,
        document_url,
        PrinterId::new(printer_id_raw),
        created_at,
        completed_at,
        error_message,
        output_path,
        settings,
    ))
}

fn completed_at_for_status(status: &PrintStatus, now: i64) -> Option<i64> {
    match status {
        PrintStatus::Completed | PrintStatus::Failed | PrintStatus::Cancelled => Some(now),
        _ => None,
    }
}

