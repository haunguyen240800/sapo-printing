//! macOS Keychain Integration Test
//!
//! Tests real macOS Keychain Services interactions.
//! Requires: macOS with user logged in, app signed (to avoid repeated prompts).

#![cfg(target_os = "macos")]

use sapo_printer::infrastructure::secrets::{MacOSKeychain, SecretManager};

struct TestCleanup<'a> {
    manager: &'a MacOSKeychain,
    key: String,
}

impl<'a> Drop for TestCleanup<'a> {
    fn drop(&mut self) {
        let _ = self.manager.delete(&self.key);
    }
}

#[test]
fn test_real_macos_store_retrieve_delete() {
    let manager = MacOSKeychain::new();
    let key = format!("integration_test_{}", uuid::Uuid::new_v4());
    let value = "integration_test_secret_value";

    let _cleanup = TestCleanup {
        manager: &manager,
        key: key.clone(),
    };

    manager
        .store(&key, value)
        .expect("Should store keychain item");

    let retrieved = manager
        .retrieve(&key)
        .expect("Should retrieve keychain item");
    assert_eq!(retrieved, Some(value.to_string()));

    manager.delete(&key).expect("Should delete keychain item");

    let after_delete = manager.retrieve(&key).expect("Should handle missing item");
    assert_eq!(after_delete, None);
}

#[test]
fn test_real_macos_overwrite_existing() {
    let manager = MacOSKeychain::new();
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
fn test_real_macos_unicode_support() {
    let manager = MacOSKeychain::new();
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
fn test_real_macos_large_value() {
    let manager = MacOSKeychain::new();
    let key = format!("integration_large_{}", uuid::Uuid::new_v4());
    let large_value = "x".repeat(5000);

    let _cleanup = TestCleanup {
        manager: &manager,
        key: key.clone(),
    };

    manager
        .store(&key, &large_value)
        .expect("Should store large value");
    let retrieved = manager.retrieve(&key).expect("Should retrieve");
    assert_eq!(retrieved, Some(large_value));
}
