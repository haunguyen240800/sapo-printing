//! Linux Secret Service Integration Test
//!
//! Tests real Secret Service API interactions via D-Bus.
//! Requires: D-Bus Secret Service daemon (gnome-keyring, kwallet, or keepassxc).
//!
//! Note: Tests will be skipped if Secret Service is unavailable (headless systems).

#![cfg(target_os = "linux")]

use sapo_printer::infrastructure::secrets::{LinuxSecretService, SecretManager};

struct TestCleanup<'a> {
    manager: &'a LinuxSecretService,
    key: String,
}

impl<'a> Drop for TestCleanup<'a> {
    fn drop(&mut self) {
        let _ = self.manager.delete(&self.key);
    }
}

fn create_manager_or_skip() -> LinuxSecretService {
    match LinuxSecretService::new() {
        Ok(manager) => manager,
        Err(e) => {
            eprintln!("Skipping test: Secret Service unavailable: {}", e);
            panic!("Secret Service unavailable - test skipped");
        }
    }
}

#[test]
#[ignore = "Requires Secret Service daemon running"]
fn test_real_linux_store_retrieve_delete() {
    let manager = create_manager_or_skip();
    let key = format!("integration_test_{}", uuid::Uuid::new_v4());
    let value = "integration_test_secret_value";

    let _cleanup = TestCleanup {
        manager: &manager,
        key: key.clone(),
    };

    manager.store(&key, value).expect("Should store secret");

    let retrieved = manager.retrieve(&key).expect("Should retrieve secret");
    assert_eq!(retrieved, Some(value.to_string()));

    manager.delete(&key).expect("Should delete secret");

    let after_delete = manager
        .retrieve(&key)
        .expect("Should handle missing secret");
    assert_eq!(after_delete, None);
}

#[test]
#[ignore = "Requires Secret Service daemon running"]
fn test_real_linux_overwrite_existing() {
    let manager = create_manager_or_skip();
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
#[ignore = "Requires Secret Service daemon running"]
fn test_real_linux_unicode_support() {
    let manager = create_manager_or_skip();
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
#[ignore = "Requires Secret Service daemon running"]
fn test_real_linux_large_value() {
    let manager = create_manager_or_skip();
    let key = format!("integration_large_{}", uuid::Uuid::new_v4());
    let large_value = "x".repeat(10000);

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

#[test]
fn test_linux_service_unavailable_error() {
    match LinuxSecretService::new() {
        Ok(_) => {
            println!("Secret Service is available");
        }
        Err(e) => {
            let error_msg = format!("{}", e);
            assert!(
                error_msg.contains("Secret service unavailable") || error_msg.contains("D-Bus")
            );
        }
    }
}
