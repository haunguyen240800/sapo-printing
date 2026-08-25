//! Application layer errors — single unified `Error` type.
//!
//! All application ports, use cases, and the print-job pipeline surface the
//! same `Error` enum.  Infrastructure implementations map their concrete error
//! types onto these variants via the `From<InfrastructureError> for Error`
//! impl in `infrastructure::error_conversions`.

use std::fmt;

use crate::domain::print_job::PrintJobError;

#[derive(Debug)]
pub enum Error {
    // ─── Port / infrastructure boundary ──────────────────────────────────────
    /// A required resource (URL, file, DB row) was not found.
    NotFound(String),
    /// The dependency is temporarily unavailable (service down, circuit open,
    /// certificate missing, etc.).
    Unavailable(String),
    /// The provided input or downloaded content failed validation.
    InvalidInput(String),
    /// The operation exceeded its time budget.
    Timeout(String),
    /// Any other operational failure originating from a port implementation.
    Operation(String),

    // ─── Use case / domain ────────────────────────────────────────────────────
    TooManyJobs {
        count: usize,
    },
    EmptyJobList,
    PrinterNotAvailable {
        name: String,
    },
    PrintJobError(PrintJobError),
    /// Persistence failure (repository, event store, etc.).
    RepositoryError(String),
    InvalidJobId {
        job_id: String,
    },
    JobNotFound {
        job_id: String,
    },
    CannotCancelCompleted {
        job_id: String,
    },
    CannotCancelFailed {
        job_id: String,
    },
    CannotCancelCancelled {
        job_id: String,
    },
    DomainRuleViolation {
        reason: String,
    },
    EventStoreError {
        reason: String,
    },
    EventBusError {
        reason: String,
    },
    ValidationError {
        reason: String,
    },
    MetricsError {
        reason: String,
    },
}

impl Error {
    /// Stable, machine-readable error code for the frontend to record and branch
    /// on. These strings are part of the API contract — keep them in sync with the
    /// HTTP status mapping in `interface::http_server::handlers::map_app_error`.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound(_) | Self::JobNotFound { .. } => "not_found",
            Self::Unavailable(_) => "service_unavailable",
            Self::InvalidInput(_) => "invalid_input",
            Self::Timeout(_) => "timeout",
            Self::Operation(_) => "operation_error",
            Self::TooManyJobs { .. } => "too_many_jobs",
            Self::EmptyJobList => "empty_job_list",
            Self::PrinterNotAvailable { .. } => "printer_not_available",
            Self::PrintJobError(_) => "print_job_error",
            Self::RepositoryError(_) => "repository_error",
            Self::InvalidJobId { .. } => "invalid_job_id",
            Self::CannotCancelCompleted { .. } => "cannot_cancel_completed",
            Self::CannotCancelFailed { .. } => "cannot_cancel_failed",
            Self::CannotCancelCancelled { .. } => "cannot_cancel_cancelled",
            Self::DomainRuleViolation { .. } => "domain_rule_violation",
            Self::EventStoreError { .. } => "event_store_error",
            Self::EventBusError { .. } => "event_bus_error",
            Self::ValidationError { .. } => "validation_error",
            Self::MetricsError { .. } => "metrics_error",
        }
    }

    /// Concise, user-facing message in Vietnamese, suitable for storing as a
    /// job's failure reason and displaying to end users.
    ///
    /// Unlike `Display`, this never embeds the raw (often long, English)
    /// infrastructure detail carried by the port variants — that detail stays
    /// in the structured logs. Use `Display`/`to_string()` for logging and
    /// `user_message()` for anything the user sees, so failure messages stay
    /// short and consistently Vietnamese.
    pub fn user_message(&self) -> String {
        match self {
            Self::NotFound(_) => "Không tìm thấy tài nguyên yêu cầu.".to_string(),
            Self::Unavailable(_) => "Dịch vụ tạm thời không khả dụng.".to_string(),
            Self::InvalidInput(_) => "Tài liệu hoặc dữ liệu đầu vào không hợp lệ.".to_string(),
            Self::Timeout(_) => "Quá thời gian chờ khi xử lý tài liệu.".to_string(),
            Self::Operation(_) => "Đã xảy ra lỗi trong quá trình in.".to_string(),
            // Remaining variants already produce concise Vietnamese messages.
            other => other.to_string(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Port variants
            Self::NotFound(msg) => write!(f, "Not found: {}", msg),
            Self::Unavailable(msg) => write!(f, "Unavailable: {}", msg),
            Self::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            Self::Timeout(msg) => write!(f, "Timeout: {}", msg),
            Self::Operation(msg) => write!(f, "Operation failed: {}", msg),

            // Use case / domain variants
            Self::TooManyJobs { count } => write!(
                f,
                "Số lượng URLs vượt quá giới hạn tối đa 5000 (nhận được: {})",
                count
            ),
            Self::EmptyJobList => write!(f, "Danh sách URLs không được rỗng"),
            Self::PrinterNotAvailable { name } => {
                write!(f, "Máy in '{}' không tồn tại hoặc không online", name)
            }
            Self::PrintJobError(e) => write!(f, "Lỗi domain: {}", e),
            Self::RepositoryError(msg) => write!(f, "Lỗi lưu trữ: {}", msg),
            Self::InvalidJobId { job_id } => write!(f, "Job ID không hợp lệ: {}", job_id),
            Self::JobNotFound { job_id } => {
                write!(f, "Không tìm thấy job với ID: {}", job_id)
            }
            Self::CannotCancelCompleted { job_id } => {
                write!(f, "Không thể hủy job đã hoàn thành: {}", job_id)
            }
            Self::CannotCancelFailed { job_id } => {
                write!(f, "Không thể hủy job đã thất bại: {}", job_id)
            }
            Self::CannotCancelCancelled { job_id } => {
                write!(f, "Không thể hủy job đã bị hủy: {}", job_id)
            }
            Self::DomainRuleViolation { reason } => {
                write!(f, "Vi phạm quy tắc nghiệp vụ: {}", reason)
            }
            Self::EventStoreError { reason } => write!(f, "Lỗi event store: {}", reason),
            Self::EventBusError { reason } => write!(f, "Lỗi event bus: {}", reason),
            Self::ValidationError { reason } => write!(f, "Lỗi validation: {}", reason),
            Self::MetricsError { reason } => write!(f, "Lỗi metrics: {}", reason),
        }
    }
}

impl std::error::Error for Error {}

impl From<PrintJobError> for Error {
    fn from(e: PrintJobError) -> Self {
        Self::PrintJobError(e)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_too_many_jobs_display() {
        let err = Error::TooManyJobs { count: 5001 };
        assert!(err.to_string().contains("5001"));
    }

    #[test]
    fn test_empty_job_list_display() {
        let err = Error::EmptyJobList;
        assert!(err.to_string().contains("không được rỗng"));
    }

    #[test]
    fn test_printer_not_available_display() {
        let err = Error::PrinterNotAvailable {
            name: "HP".to_string(),
        };
        assert!(err.to_string().contains("HP"));
    }

    #[test]
    fn test_domain_error_wraps_domain_error() {
        let domain_err = PrintJobError::MaxRetryExceeded;
        let app_err = Error::from(domain_err.clone());
        assert!(matches!(app_err, Error::PrintJobError(e) if e == domain_err));
    }

    #[test]
    fn test_repository_error_display() {
        let err = Error::RepositoryError("DB connection failed".to_string());
        assert!(err.to_string().contains("DB connection failed"));
    }

    #[test]
    fn test_port_not_found_display() {
        let err = Error::NotFound("resource X".to_string());
        assert!(err.to_string().contains("resource X"));
    }

    #[test]
    fn test_error_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Error>();
    }
}
