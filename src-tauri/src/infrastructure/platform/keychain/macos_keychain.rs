//! macOS Keychain Implementation
//!
//! Uses Keychain Services via the `security-framework` crate for secure secret storage.
//! Secrets are stored in the user's default keychain (login.keychain-db).
//!
//! # Limitations
//! - App must be signed to avoid repeated keychain access prompts
//! - First-time access prompts user for consent
//! - View items: Keychain Access.app → login → Passwords
//!
//! # Service/Account Structure
//! - Service: `"com.sapo.printer"` (constant)
//! - Account: `{key}` (logical key name)

#[cfg(target_os = "macos")]
use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};

use crate::infrastructure::platform::keychain::SecretManager;
use crate::shared::errors::InfrastructureError;

/// macOS Keychain implementation using Keychain Services.
#[cfg(target_os = "macos")]
pub struct MacOSKeychain;

#[cfg(target_os = "macos")]
impl MacOSKeychain {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(target_os = "macos")]
impl SecretManager for MacOSKeychain {
    fn store(&self, key: &str, value: &str) -> Result<(), InfrastructureError> {
        // Validate key format
        super::validate_key(key)?;

        // Validate size limit (apply Windows limit for consistency)
        if value.len() > super::MAX_SECRET_SIZE {
            return Err(InfrastructureError::SecretStoreError(format!(
                "Secret value too large: {} bytes (max {} bytes)",
                value.len(),
                super::MAX_SECRET_SIZE
            )));
        }

        // Try to store, handle duplicate by updating
        match set_generic_password("com.sapo.printer", key, value.as_bytes()) {
            Ok(_) => Ok(()),
            Err(e) => {
                // errSecDuplicateItem (-25299) means key exists, delete and retry
                if e.code() == -25299 {
                    delete_generic_password("com.sapo.printer", key).map_err(|_| {
                        InfrastructureError::SecretStoreError(format!(
                            "Failed to delete existing keychain item '{}' before update",
                            key
                        ))
                    })?;
                    set_generic_password("com.sapo.printer", key, value.as_bytes()).map_err(|e| {
                        InfrastructureError::SecretStoreError(format!(
                            "Failed to update keychain item '{}': {:?}",
                            key, e
                        ))
                    })
                } else {
                    Err(InfrastructureError::SecretStoreError(format!(
                        "Failed to store keychain item '{}': {:?}",
                        key, e
                    )))
                }
            }
        }
    }

    fn retrieve(&self, key: &str) -> Result<Option<String>, InfrastructureError> {
        match get_generic_password("com.sapo.printer", key) {
            Ok(password_bytes) => {
                let value = String::from_utf8(password_bytes).map_err(|e| {
                    InfrastructureError::SecretRetrieveError(format!(
                        "Invalid UTF-8 in keychain item '{}': {}",
                        key, e
                    ))
                })?;
                Ok(Some(value))
            }
            Err(e) => {
                if e.code() == -25300 {
                    Ok(None)
                } else {
                    Err(InfrastructureError::SecretRetrieveError(format!(
                        "Failed to retrieve keychain item '{}': {:?}",
                        key, e
                    )))
                }
            }
        }
    }

    fn delete(&self, key: &str) -> Result<(), InfrastructureError> {
        match delete_generic_password("com.sapo.printer", key) {
            Ok(_) => Ok(()),
            Err(e) => {
                if e.code() == -25300 {
                    Ok(())
                } else {
                    Err(InfrastructureError::SecretDeleteError(format!(
                        "Failed to delete keychain item '{}': {:?}",
                        key, e
                    )))
                }
            }
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn test_store_retrieve_roundtrip() {
        let manager = MacOSKeychain::new();
        let key = format!("test_roundtrip_{}", uuid::Uuid::new_v4());
        let value = "test_secret_value_123";

        manager.store(&key, value).unwrap();
        let retrieved = manager.retrieve(&key).unwrap();
        assert_eq!(retrieved, Some(value.to_string()));

        manager.delete(&key).unwrap();
    }

    #[test]
    fn test_delete_removes_secret() {
        let manager = MacOSKeychain::new();
        let key = format!("test_delete_{}", uuid::Uuid::new_v4());
        let value = "to_be_deleted";

        manager.store(&key, value).unwrap();
        manager.delete(&key).unwrap();

        let retrieved = manager.retrieve(&key).unwrap();
        assert_eq!(retrieved, None);
    }

    #[test]
    fn test_retrieve_nonexistent_returns_none() {
        let manager = MacOSKeychain::new();
        let key = format!("nonexistent_{}", uuid::Uuid::new_v4());

        let result = manager.retrieve(&key).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_namespace_isolation() {
        let manager = MacOSKeychain::new();
        let key = format!("namespace_test_{}", uuid::Uuid::new_v4());
        let value = "isolated_value";

        manager.store(&key, value).unwrap();

        manager.delete(&key).unwrap();
    }

    #[test]
    fn test_delete_nonexistent_succeeds() {
        let manager = MacOSKeychain::new();
        let key = format!("never_existed_{}", uuid::Uuid::new_v4());

        let result = manager.delete(&key);
        assert!(result.is_ok());
    }
}
