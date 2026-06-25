use crate::domain::print_job::errors::DomainError;
use std::fmt;

/// Application-layer error enum for use case operations.
///
/// Implemented manually (no `thiserror`) to match existing `DomainError` pattern.
#[derive(Debug)]
pub enum ApplicationError {
    TooManyJobs { count: usize },
    EmptyJobList,
    PrinterNotAvailable { name: String },
    DomainError(DomainError),
    RepositoryError(String),
    InvalidJobId { job_id: String },
    JobNotFound { job_id: String },
    CannotCancelCompleted { job_id: String },
    CannotCancelFailed { job_id: String },
    CannotCancelCancelled { job_id: String },
    DomainRuleViolation { reason: String },
    EventStoreError { reason: String },
    EventBusError { reason: String },
    ValidationError { reason: String },
    MetricsError { reason: String },
}

impl fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyJobs { count } => write!(
                f,
                "Số lượng URLs vượt quá giới hạn tối đa 5000 (nhận được: {})",
                count
            ),
            Self::EmptyJobList => write!(f, "Danh sách URLs không được rỗng"),
            Self::PrinterNotAvailable { name } => {
                write!(f, "Máy in '{}' không tồn tại hoặc không online", name)
            }
            Self::DomainError(e) => write!(f, "Lỗi domain: {}", e),
            Self::RepositoryError(msg) => write!(f, "Lỗi lưu trữ: {}", msg),
            Self::InvalidJobId { job_id } => write!(f, "Job ID không hợp lệ: {}", job_id),
            Self::JobNotFound { job_id } => write!(f, "Không tìm thấy job với ID: {}", job_id),
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

impl std::error::Error for ApplicationError {}

impl From<DomainError> for ApplicationError {
    fn from(e: DomainError) -> Self {
        Self::DomainError(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_too_many_jobs_display() {
        let err = ApplicationError::TooManyJobs { count: 5001 };
        assert!(err.to_string().contains("5001"));
    }

    #[test]
    fn test_empty_job_list_display() {
        let err = ApplicationError::EmptyJobList;
        assert!(err.to_string().contains("không được rỗng"));
    }

    #[test]
    fn test_printer_not_available_display() {
        let err = ApplicationError::PrinterNotAvailable {
            name: "HP".to_string(),
        };
        assert!(err.to_string().contains("HP"));
    }

    #[test]
    fn test_domain_error_wraps_domain_error() {
        let domain_err = DomainError::MaxRetryExceeded;
        let app_err = ApplicationError::from(domain_err.clone());
        assert!(matches!(app_err, ApplicationError::DomainError(e) if e == domain_err));
    }

    #[test]
    fn test_repository_error_display() {
        let err = ApplicationError::RepositoryError("DB connection failed".to_string());
        assert!(err.to_string().contains("DB connection failed"));
    }

    #[test]
    fn test_application_error_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ApplicationError>();
    }
}
