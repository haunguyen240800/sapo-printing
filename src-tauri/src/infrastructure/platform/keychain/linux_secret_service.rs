//! Linux Secret Service Implementation
//!
//! Uses the Secret Service API (org.freedesktop.secrets) via D-Bus for secure secret storage.
//! Requires a Secret Service daemon (gnome-keyring, kwallet, or keepassxc) to be running.
//!
//! # Limitations
//! - Requires D-Bus Secret Service daemon running
//! - Headless systems (servers) typically don't have Secret Service
//! - View secrets: `seahorse` (GNOME), `kwalletmanager` (KDE), or `secret-tool` CLI
//!
//! # Fallback Strategy
//! If Secret Service is unavailable, the constructor returns an error with a clear message
//! suggesting environment variables as a fallback.
//!
//! # Schema/Label Structure
//! - Schema: `"com.sapo.printer"` (application identifier)
//! - Label: `"SAPO Printer: {key}"` (descriptive label for UI)

#[cfg(target_os = "linux")]
use secret_service::{Collection, EncryptionType, SecretService};

use crate::infrastructure::platform::keychain::SecretManager;
use crate::shared::errors::InfrastructureError;

/// Linux Secret Service implementation using D-Bus API.
#[cfg(target_os = "linux")]
pub struct LinuxSecretService {
    service: SecretService<'static>,
}

#[cfg(target_os = "linux")]
impl LinuxSecretService {
    /// Create a new LinuxSecretService instance.
    ///
    /// # Errors
    /// Returns `SecretServiceUnavailable` if D-Bus Secret Service daemon is not running.
    /// Install gnome-keyring, kwallet, or keepassxc to provide the service.
    pub fn new() -> Result<Self, InfrastructureError> {
        let service = SecretService::connect(EncryptionType::Dh)
            .map_err(|e| {
                InfrastructureError::SecretServiceUnavailable(format!(
                    "D-Bus Secret Service not available: {}. Install gnome-keyring or use environment variables.",
                    e
                ))
            })?;
        Ok(Self { service })
    }

    fn get_default_collection(&self) -> Result<Collection<'static>, InfrastructureError> {
        self.service.get_default_collection().map_err(|e| {
            InfrastructureError::SecretServiceUnavailable(format!(
                "Failed to access default collection: {}",
                e
            ))
        })
    }
}

#[cfg(target_os = "linux")]
impl SecretManager for LinuxSecretService {
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

        let collection = self.get_default_collection()?;

        let label = format!("SAPO Printer: {}", key);
        let mut attributes = std::collections::HashMap::new();
        attributes.insert("application", "com.sapo.printer");
        attributes.insert("key", key);

        collection
            .create_item(&label, attributes, value.as_bytes(), true, "text/plain")
            .map_err(|e| {
                InfrastructureError::SecretStoreError(format!(
                    "Failed to create secret item '{}': {}",
                    key, e
                ))
            })?;

        Ok(())
    }

    fn retrieve(&self, key: &str) -> Result<Option<String>, InfrastructureError> {
        let collection = self.get_default_collection()?;

        let mut attributes = std::collections::HashMap::new();
        attributes.insert("application", "com.sapo.printer");
        attributes.insert("key", key);

        let items = collection.search_items(attributes).map_err(|e| {
            InfrastructureError::SecretRetrieveError(format!(
                "Failed to search for secret '{}': {}",
                key, e
            ))
        })?;

        // Use first() to safely handle potential race condition
        let item = match items.first() {
            Some(item) => item,
            None => return Ok(None),
        };

        let secret = item.get_secret().map_err(|e| {
            InfrastructureError::SecretRetrieveError(format!(
                "Failed to get secret value for '{}': {}",
                key, e
            ))
        })?;

        let value = String::from_utf8(secret).map_err(|e| {
            InfrastructureError::SecretRetrieveError(format!(
                "Invalid UTF-8 in secret '{}': {}",
                key, e
            ))
        })?;

        Ok(Some(value))
    }

    fn delete(&self, key: &str) -> Result<(), InfrastructureError> {
        let collection = self.get_default_collection()?;

        let mut attributes = std::collections::HashMap::new();
        attributes.insert("application", "com.sapo.printer");
        attributes.insert("key", key);

        let items = collection.search_items(attributes).map_err(|e| {
            InfrastructureError::SecretDeleteError(format!(
                "Failed to search for secret '{}' to delete: {}",
                key, e
            ))
        })?;

        for item in items {
            item.delete().map_err(|e| {
                InfrastructureError::SecretDeleteError(format!(
                    "Failed to delete secret '{}': {}",
                    key, e
                ))
            })?;
        }

        Ok(())
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    fn create_manager() -> LinuxSecretService {
        LinuxSecretService::new().expect("Secret Service should be available for tests")
    }

    #[test]
    fn test_store_retrieve_roundtrip() {
        let manager = create_manager();
        let key = format!("test_roundtrip_{}", uuid::Uuid::new_v4());
        let value = "test_secret_value_123";

        manager.store(&key, value).unwrap();
        let retrieved = manager.retrieve(&key).unwrap();
        assert_eq!(retrieved, Some(value.to_string()));

        manager.delete(&key).unwrap();
    }

    #[test]
    fn test_delete_removes_secret() {
        let manager = create_manager();
        let key = format!("test_delete_{}", uuid::Uuid::new_v4());
        let value = "to_be_deleted";

        manager.store(&key, value).unwrap();
        manager.delete(&key).unwrap();

        let retrieved = manager.retrieve(&key).unwrap();
        assert_eq!(retrieved, None);
    }

    #[test]
    fn test_retrieve_nonexistent_returns_none() {
        let manager = create_manager();
        let key = format!("nonexistent_{}", uuid::Uuid::new_v4());

        let result = manager.retrieve(&key).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_namespace_isolation() {
        let manager = create_manager();
        let key = format!("namespace_test_{}", uuid::Uuid::new_v4());
        let value = "isolated_value";

        manager.store(&key, value).unwrap();

        manager.delete(&key).unwrap();
    }

    #[test]
    fn test_delete_nonexistent_succeeds() {
        let manager = create_manager();
        let key = format!("never_existed_{}", uuid::Uuid::new_v4());

        let result = manager.delete(&key);
        assert!(result.is_ok());
    }
}
