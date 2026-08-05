use crate::shared::errors::InfrastructureError;

pub fn is_retryable(error: &InfrastructureError) -> bool {
    match error {
        InfrastructureError::NetworkError(_)
        | InfrastructureError::TimeoutError(_)
        | InfrastructureError::CircuitOpenError
        | InfrastructureError::RenderError(_)
        | InfrastructureError::PrinterError { .. } => true,

        InfrastructureError::ValidationError(_)
        | InfrastructureError::DatabaseError { .. }
        | InfrastructureError::SecretStoreError(_)
        | InfrastructureError::SecretRetrieveError(_)
        | InfrastructureError::SecretDeleteError(_)
        | InfrastructureError::SecretServiceUnavailable(_)
        | InfrastructureError::TlsError(_)
        | InfrastructureError::TlsCertUnavailable(_)
        | InfrastructureError::IpcError(_)
        | InfrastructureError::BindError(_)
        | InfrastructureError::IoError(_)
        | InfrastructureError::SerializationError(_) => false,
    }
}

/// Exponential backoff schedule (from FR-1.4):
/// - retry_count = 0 → 5s
/// - retry_count = 1 → 10s
/// - retry_count = 2 → 20s
pub fn calculate_backoff_delay(retry_count: u32) -> u64 {
    match retry_count {
        0 => 5,
        1 => 10,
        2 => 20,
        _ => {
            tracing::warn!(
                target = "sapo_printer::application::policy::retry",
                retry_count = retry_count,
                "calculate_backoff_delay called with out-of-range retry_count, using max"
            );
            20
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_error_is_retryable() {
        assert!(is_retryable(&InfrastructureError::NetworkError("x".into())));
    }

    #[test]
    fn test_timeout_error_is_retryable() {
        assert!(is_retryable(&InfrastructureError::TimeoutError("x".into())));
    }

    #[test]
    fn test_circuit_open_is_retryable() {
        assert!(is_retryable(&InfrastructureError::CircuitOpenError));
    }

    #[test]
    fn test_render_error_is_retryable() {
        assert!(is_retryable(&InfrastructureError::RenderError("x".into())));
    }

    #[test]
    fn test_printer_error_is_retryable() {
        assert!(is_retryable(&InfrastructureError::PrinterError {
            reason: "busy".into(),
        }));
    }

    #[test]
    fn test_validation_error_is_not_retryable() {
        assert!(!is_retryable(&InfrastructureError::ValidationError("x".into())));
    }

    #[test]
    fn test_database_error_is_not_retryable() {
        assert!(!is_retryable(&InfrastructureError::DatabaseError {
            reason: "x".into(),
        }));
    }

    #[test]
    fn test_secret_store_error_is_not_retryable() {
        assert!(!is_retryable(&InfrastructureError::SecretStoreError("x".into())));
    }

    #[test]
    fn test_backoff_delay_schedule() {
        assert_eq!(calculate_backoff_delay(0), 5);
        assert_eq!(calculate_backoff_delay(1), 10);
        assert_eq!(calculate_backoff_delay(2), 20);
    }

    #[test]
    fn test_backoff_delay_beyond_max() {
        assert_eq!(calculate_backoff_delay(3), 20);
        assert_eq!(calculate_backoff_delay(10), 20);
    }
}
