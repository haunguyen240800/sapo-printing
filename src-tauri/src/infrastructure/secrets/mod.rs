//! Secret Management Module
//!
//! Platform-agnostic secret storage using OS-native credential managers:
//! - Windows: Credential Manager (DPAPI-backed)
//! - macOS: Keychain Services
//! - Linux: Secret Service API (D-Bus)
//!
//! # Architecture
//! Uses the Strategy pattern with compile-time dispatch via conditional compilation.
//! Only the platform-specific implementation for the target OS is compiled.

mod secret_manager;

pub use secret_manager::{format_key, validate_key, SecretManager, MAX_SECRET_SIZE};

#[cfg(target_os = "windows")]
mod windows_credential_manager;
#[cfg(target_os = "windows")]
pub use windows_credential_manager::WindowsCredentialManager;

#[cfg(target_os = "macos")]
mod macos_keychain;
#[cfg(target_os = "macos")]
pub use macos_keychain::MacOSKeychain;

#[cfg(target_os = "linux")]
mod linux_secret_service;
#[cfg(target_os = "linux")]
pub use linux_secret_service::LinuxSecretService;
