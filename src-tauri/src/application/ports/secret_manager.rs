//! SecretManager port — application abstraction over OS-native secret storage.
//!
//! Use cases depend on this trait; concrete implementations (Windows Credential
//! Manager, macOS Keychain, Linux Secret Service) live in the infrastructure layer.
//!
//! Helpers for key formatting/validation live in
//! `application::services::secret_key_service`.

use crate::shared::errors::InfrastructureError;

/// Platform-agnostic trait for secure secret storage.
pub trait SecretManager: Send + Sync {
    /// Store a secret value associated with a key.
    fn store(&self, key: &str, value: &str) -> Result<(), InfrastructureError>;

    /// Retrieve a secret value by key.
    ///
    /// Returns `Ok(None)` when the key does not exist (not an error).
    fn retrieve(&self, key: &str) -> Result<Option<String>, InfrastructureError>;

    /// Delete a secret by key. Succeeds even if the key did not exist.
    fn delete(&self, key: &str) -> Result<(), InfrastructureError>;
}
