//! Helpers around secret key naming, validation, and platform constraints.
//!
//! Kept separate from the `SecretManager` port so the port contains only the
//! trait. Consumed by both the port's preconditions and the infrastructure
//! keychain adapters.

use crate::shared::errors::InfrastructureError;

/// Maximum secret size supported across all platforms (Windows limit).
pub const MAX_SECRET_SIZE: usize = 2560;

/// Format a key with the application namespace `com.sapo.printer/{key}`.
///
/// # Panics
/// Panics if key contains '/' character (namespace separator).
pub fn format_key(key: &str) -> String {
    if key.contains('/') {
        panic!("Key cannot contain '/' character: {}", key);
    }
    format!("com.sapo.printer/{}", key)
}

/// Validate a key name for use with SecretManager.
pub fn validate_key(key: &str) -> Result<(), InfrastructureError> {
    if key.is_empty() {
        return Err(InfrastructureError::SecretStoreError(
            "Key cannot be empty".to_string(),
        ));
    }
    if key.contains('/') {
        return Err(InfrastructureError::SecretStoreError(format!(
            "Key cannot contain '/' character: {}",
            key
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_key_adds_namespace() {
        assert_eq!(format_key("device_token"), "com.sapo.printer/device_token");
        assert_eq!(format_key("hmac_key"), "com.sapo.printer/hmac_key");
    }

    #[test]
    fn test_format_key_handles_empty_string() {
        assert_eq!(format_key(""), "com.sapo.printer/");
    }

    #[test]
    fn test_format_key_handles_special_chars() {
        assert_eq!(format_key("user:token"), "com.sapo.printer/user:token");
    }

    #[test]
    fn test_validate_key_rejects_empty() {
        assert!(validate_key("").is_err());
    }

    #[test]
    fn test_validate_key_rejects_slash() {
        assert!(validate_key("a/b").is_err());
    }

    #[test]
    fn test_validate_key_accepts_normal() {
        assert!(validate_key("device_token").is_ok());
    }
}
