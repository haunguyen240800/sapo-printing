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
