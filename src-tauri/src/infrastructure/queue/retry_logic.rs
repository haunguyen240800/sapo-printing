use crate::shared::errors::InfrastructureError;

/// Classify error as retryable or non-retryable.
///
/// **Retryable errors:** Transient failures that may succeed on retry
/// - NetworkError (timeout, connection refused, DNS failure)
/// - TimeoutError (request timeout, IO timeout)
/// - CircuitOpenError (circuit breaker protecting service)
/// - RenderError (PDF corruption, MuPDF crash — may work with retry)
/// - PrinterError (printer busy, out of paper — may recover)
///
/// **Non-retryable errors:** Permanent failures that will never succeed
/// - ValidationError (invalid PDF, malformed data)
/// - DatabaseError (schema error, constraint violation)
/// - SecretStoreError / SecretRetrieveError (auth/config issues)
///
/// # Returns
/// `true` if error should trigger retry, `false` if job should fail immediately.
pub fn is_retryable(error: &InfrastructureError) -> bool {
    match error {
        // Retryable — transient network/service failures
        InfrastructureError::NetworkError(_) => true,
        InfrastructureError::TimeoutError(_) => true,
        InfrastructureError::CircuitOpenError => true,

        // Retryable — rendering may succeed on retry (unstable MuPDF)
        InfrastructureError::RenderError(_) => true,

        // Retryable — printer may recover (busy, out of paper, warming up)
        InfrastructureError::PrinterError { .. } => true,

        // Non-retryable — data validation will always fail
        InfrastructureError::ValidationError(_) => false,

        // Non-retryable — database schema/constraint issues
        InfrastructureError::DatabaseError { .. } => false,

        // Non-retryable — auth/config issues require manual fix
        InfrastructureError::SecretStoreError(_) => false,
        InfrastructureError::SecretRetrieveError(_) => false,
        InfrastructureError::SecretDeleteError(_) => false,
        InfrastructureError::SecretServiceUnavailable(_) => false,
    }
}

/// Calculate exponential backoff delay in seconds.
///
/// Backoff schedule (from FR-1.4):
/// - retry_count = 0 → 5s
/// - retry_count = 1 → 10s
/// - retry_count = 2 → 20s
///
/// # Arguments
/// * `retry_count` - Current retry attempt (0-based: 0 = first retry)
///
/// # Returns
/// Delay in seconds before next retry attempt.
///
/// # Panics
/// Panics in debug builds if retry_count > 2 (invalid state, MAX_RETRY_COUNT = 3).
pub fn calculate_backoff_delay(retry_count: u32) -> u64 {
    debug_assert!(
        retry_count <= 2,
        "calculate_backoff_delay called with retry_count={}, max valid is 2",
        retry_count
    );

    match retry_count {
        0 => 5,  // First retry: 5s
        1 => 10, // Second retry: 10s
        2 => 20, // Third retry: 20s
        _ => {
            // Fallback for out-of-bounds retry_count (should never happen in production)
            // but provides safe default if MAX_RETRY_COUNT enforcement fails
            eprintln!(
                "Warning: calculate_backoff_delay called with out-of-range retry_count={}",
                retry_count
            );
            20
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::errors::InfrastructureError;

    #[test]
    fn test_network_error_is_retryable() {
        let error = InfrastructureError::NetworkError("Connection refused".into());
        assert!(is_retryable(&error));
    }

    #[test]
    fn test_timeout_error_is_retryable() {
        let error = InfrastructureError::TimeoutError("Request timeout".into());
        assert!(is_retryable(&error));
    }

    #[test]
    fn test_circuit_open_is_retryable() {
        let error = InfrastructureError::CircuitOpenError;
        assert!(is_retryable(&error));
    }

    #[test]
    fn test_render_error_is_retryable() {
        let error = InfrastructureError::RenderError("MuPDF crash".into());
        assert!(is_retryable(&error));
    }

    #[test]
    fn test_printer_error_is_retryable() {
        let error = InfrastructureError::PrinterError {
            reason: "Printer busy".into(),
        };
        assert!(is_retryable(&error));
    }

    #[test]
    fn test_validation_error_is_not_retryable() {
        let error = InfrastructureError::ValidationError("Invalid PDF".into());
        assert!(!is_retryable(&error));
    }

    #[test]
    fn test_database_error_is_not_retryable() {
        let error = InfrastructureError::DatabaseError {
            reason: "Schema error".into(),
        };
        assert!(!is_retryable(&error));
    }

    #[test]
    fn test_secret_store_error_is_not_retryable() {
        let error = InfrastructureError::SecretStoreError("Auth failed".into());
        assert!(!is_retryable(&error));
    }

    #[test]
    fn test_backoff_delay_first_retry() {
        assert_eq!(calculate_backoff_delay(0), 5);
    }

    #[test]
    fn test_backoff_delay_second_retry() {
        assert_eq!(calculate_backoff_delay(1), 10);
    }

    #[test]
    fn test_backoff_delay_third_retry() {
        assert_eq!(calculate_backoff_delay(2), 20);
    }

    #[test]
    fn test_backoff_delay_beyond_max() {
        // Should not happen (MAX_RETRY_COUNT = 3), but test fallback
        assert_eq!(calculate_backoff_delay(3), 20);
        assert_eq!(calculate_backoff_delay(10), 20);
    }
}
