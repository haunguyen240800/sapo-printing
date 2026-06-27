//! Secret Manager Trait
//!
//! Provides a platform-agnostic abstraction for secure storage of sensitive data
//! (device tokens, HMAC signing keys) using OS-native security mechanisms:
//! - Windows: Credential Manager (DPAPI-backed)
//! - macOS: Keychain Services
//! - Linux: Secret Service API (D-Bus)
//!
//! All implementations use the namespace `"com.sapo.printer"` to prevent conflicts
//! with other applications.
//!
//! # Thread Safety
//! All implementations must be `Send + Sync` for use with `Arc<dyn SecretManager>`.
//! Operations are thread-safe at the OS level, but concurrent modifications to the
//! same key from multiple threads may result in last-write-wins behavior with no
//! atomicity guarantees across delete+store sequences.
//!
//! # Concurrent Access
//! Multiple threads can safely call store/retrieve/delete on different keys concurrently.
//! For the same key, OS APIs provide atomic single-operation guarantees but not
//! transaction-level atomicity. If your use case requires atomic read-modify-write,
//! implement application-level locking.
//!
//! # Error Semantics
//! - `Ok(None)` — Key doesn't exist (normal case, not an error)
//! - `Err(...)` — OS keychain unavailable, permission denied, or system failure
//!
//! # Security Considerations
//! Error messages include key names for debuggability. Avoid using sensitive data
//! as key names (e.g., tokens, PII). Use descriptive identifiers like "device_token"
//! or "hmac_key_{device_id}" instead of embedding actual secret values in key names.
//!
//! # Size Limits
//! - Windows: 2560 bytes maximum (CRED_MAX_CREDENTIAL_BLOB_SIZE)
//! - macOS: ~4KB typical, 64KB system limit
//! - Linux: D-Bus message size limit (~128MB, practically unlimited)

use crate::shared::errors::InfrastructureError;

/// Maximum secret size supported across all platforms (Windows limit)
pub const MAX_SECRET_SIZE: usize = 2560;

/// Platform-agnostic trait for secure secret storage.
///
/// Implementations use OS-native credential storage:
/// - Windows: Credential Manager (Win32 API)
/// - macOS: Keychain Services
/// - Linux: Secret Service (D-Bus)
pub trait SecretManager: Send + Sync {
    /// Store a secret value associated with a key.
    ///
    /// # Arguments
    /// * `key` - Logical key name (will be namespaced automatically)
    /// * `value` - Secret value to store (max 2560 bytes)
    ///
    /// # Returns
    /// `Ok(())` on success, `Err(SecretStoreError)` on OS failure or if value exceeds size limit
    ///
    /// # Errors
    /// Returns `SecretStoreError` if:
    /// - Value exceeds 2560 bytes (Windows CRED_MAX_CREDENTIAL_BLOB_SIZE)
    /// - OS keychain operation fails
    fn store(&self, key: &str, value: &str) -> Result<(), InfrastructureError>;

    /// Retrieve a secret value by key.
    ///
    /// # Arguments
    /// * `key` - Logical key name (will be namespaced automatically)
    ///
    /// # Returns
    /// - `Ok(Some(value))` — Secret found (empty string is a valid secret)
    /// - `Ok(None)` — Key doesn't exist (NOT an error)
    /// - `Err(SecretRetrieveError)` — OS failure or permission denied
    ///
    /// # Note
    /// An empty string (`""`) is considered a valid secret value, distinct from
    /// a missing key. If you need to distinguish between empty and missing, check
    /// for `None` vs `Some("")`.
    fn retrieve(&self, key: &str) -> Result<Option<String>, InfrastructureError>;

    /// Delete a secret by key.
    ///
    /// # Arguments
    /// * `key` - Logical key name (will be namespaced automatically)
    ///
    /// # Returns
    /// `Ok(())` on success (even if key didn't exist), `Err(SecretDeleteError)` on OS failure
    fn delete(&self, key: &str) -> Result<(), InfrastructureError>;
}

/// Format a key with the application namespace.
///
/// # Arguments
/// * `key` - Logical key name (e.g., "device_token")
///
/// # Returns
/// Namespaced key in format `"com.sapo.printer/{key}"`
///
/// # Panics
/// Panics if key contains '/' character (namespace separator)
///
/// # Example
/// ```
/// # use sapo_printer::infrastructure::secrets::format_key;
/// assert_eq!(format_key("device_token"), "com.sapo.printer/device_token");
/// ```
pub fn format_key(key: &str) -> String {
    if key.contains('/') {
        panic!("Key cannot contain '/' character: {}", key);
    }
    format!("com.sapo.printer/{}", key)
}

/// Validate a key name for use with SecretManager.
///
/// # Arguments
/// * `key` - Logical key name to validate
///
/// # Returns
/// `Ok(())` if key is valid, `Err(SecretStoreError)` if invalid
///
/// # Errors
/// Returns error if:
/// - Key contains '/' character (namespace separator)
/// - Key is empty
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
}
