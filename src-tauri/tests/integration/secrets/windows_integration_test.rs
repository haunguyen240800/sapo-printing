//! Windows Credential Manager Integration Test
//!
//! Tests real Windows Credential Manager API interactions.
//! Requires: Windows OS with user logged in.

#![cfg(target_os = "windows")]

use sapo_printer::infrastructure::secrets::{SecretManager, WindowsCredentialManager};

struct TestCleanup<'a> {
    manager: &'a WindowsCredentialManager,
    key: String,
}

impl<'a> Drop for TestCleanup<'a> {
    fn drop(&mut self) {
        let _ = self.manager.delete(&self.key);
    }
}

#[test]
fn test_real_windows_store_retrieve_delete() {
    let manager = WindowsCredentialManager::new();
    let key = format!("integration_test_{}", uuid::Uuid::new_v4());
    let value = "integration_test_secret_value";

    let _cleanup = TestCleanup {
        manager: &manager,
        key: key.clone(),
    };

    manager.store(&key, value).expect("Should store credential");

    let retrieved = manager.retrieve(&key).expect("Should retrieve credential");
    assert_eq!(retrieved, Some(value.to_string()));

    manager.delete(&key).expect("Should delete credential");

    let after_delete = manager
        .retrieve(&key)
        .expect("Should handle missing credential");
    assert_eq!(after_delete, None);
}

#[test]
fn test_real_windows_overwrite_existing() {
    let manager = WindowsCredentialManager::new();
    let key = format!("integration_overwrite_{}", uuid::Uuid::new_v4());

    let _cleanup = TestCleanup {
        manager: &manager,
        key: key.clone(),
    };

    manager
        .store(&key, "first_value")
        .expect("Should store first value");
    manager
        .store(&key, "second_value")
        .expect("Should overwrite with second value");

    let retrieved = manager.retrieve(&key).expect("Should retrieve");
    assert_eq!(retrieved, Some("second_value".to_string()));
}

#[test]
fn test_real_windows_unicode_support() {
    let manager = WindowsCredentialManager::new();
    let key = format!("integration_unicode_{}", uuid::Uuid::new_v4());
    let unicode_value = "Tiếng Việt 中文 日本語 🔐";

    let _cleanup = TestCleanup {
        manager: &manager,
        key: key.clone(),
    };

    manager
        .store(&key, unicode_value)
        .expect("Should store unicode");
    let retrieved = manager.retrieve(&key).expect("Should retrieve unicode");
    assert_eq!(retrieved, Some(unicode_value.to_string()));
}

#[test]
fn test_real_windows_large_value() {
    let manager = WindowsCredentialManager::new();
    let key = format!("integration_large_{}", uuid::Uuid::new_v4());
    let large_value = "x".repeat(2000);

    let _cleanup = TestCleanup {
        manager: &manager,
        key: key.clone(),
    };

    manager
        .store(&key, &large_value)
        .expect("Should store large value (under 2560 bytes limit)");
    let retrieved = manager.retrieve(&key).expect("Should retrieve");
    assert_eq!(retrieved, Some(large_value));
}
