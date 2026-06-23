//! Windows Credential Manager Implementation
//!
//! Uses Win32 Credentials API (DPAPI-backed) for secure secret storage.
//! Credentials are stored per-user and encrypted by Windows Data Protection API.
//!
//! # Limitations
//! - Requires user to be logged in (doesn't work with service accounts)
//! - Max credential size: 2560 bytes
//! - View credentials: Control Panel → Credential Manager → Windows Credentials
//!
//! # API Reference
//! - `CredWriteW()` — Store credential
//! - `CredReadW()` — Retrieve credential
//! - `CredDeleteW()` — Delete credential

#[cfg(target_os = "windows")]
use windows::core::{PCWSTR, PWSTR};
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::ERROR_NOT_FOUND;
#[cfg(target_os = "windows")]
use windows::Win32::Security::Credentials::{
    CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_FLAGS,
    CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
};

use crate::infrastructure::secrets::SecretManager;
use crate::shared::errors::InfrastructureError;

/// Windows Credential Manager implementation using Win32 API.
#[cfg(target_os = "windows")]
pub struct WindowsCredentialManager;

#[cfg(target_os = "windows")]
impl WindowsCredentialManager {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(target_os = "windows")]
impl SecretManager for WindowsCredentialManager {
    fn store(&self, key: &str, value: &str) -> Result<(), InfrastructureError> {
        // Validate key format
        super::validate_key(key)?;

        // Validate size limit (CRED_MAX_CREDENTIAL_BLOB_SIZE = 2560 bytes)
        if value.len() > super::MAX_SECRET_SIZE {
            return Err(InfrastructureError::SecretStoreError(format!(
                "Secret value too large: {} bytes (max {} bytes)",
                value.len(),
                super::MAX_SECRET_SIZE
            )));
        }

        let target_name = super::format_key(key);

        // Validate target name length (CRED_MAX_STRING_LENGTH = 256)
        let target_name_wide: Vec<u16> = target_name.encode_utf16().chain(Some(0)).collect();
        if target_name_wide.len() > 256 {
            return Err(InfrastructureError::SecretStoreError(format!(
                "Target name too long: {} chars (max 256)",
                target_name_wide.len()
            )));
        }

        let value_bytes = value.as_bytes();

        let mut credential = CREDENTIALW {
            Flags: CRED_FLAGS(0),
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target_name_wide.as_ptr() as *mut u16),
            Comment: PWSTR::null(),
            LastWritten: Default::default(),
            CredentialBlobSize: value_bytes.len() as u32,
            CredentialBlob: value_bytes.as_ptr() as *mut u8,
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            AttributeCount: 0,
            Attributes: std::ptr::null_mut(),
            TargetAlias: PWSTR::null(),
            UserName: PWSTR::null(),
        };

        unsafe {
            CredWriteW(&mut credential, 0).map_err(|e| {
                InfrastructureError::SecretStoreError(format!(
                    "Failed to write credential '{}': {}",
                    key,
                    e.message()
                ))
            })?;
        }

        Ok(())
    }

    fn retrieve(&self, key: &str) -> Result<Option<String>, InfrastructureError> {
        let target_name = super::format_key(key);
        let target_name_wide: Vec<u16> = target_name.encode_utf16().chain(Some(0)).collect();

        let mut p_credential: *mut CREDENTIALW = std::ptr::null_mut();

        unsafe {
            match CredReadW(
                PCWSTR(target_name_wide.as_ptr()),
                CRED_TYPE_GENERIC,
                0,
                &mut p_credential as *mut _,
            ) {
                Ok(_) => {
                    let credential = &*p_credential;

                    // Check for null CredentialBlob pointer
                    if credential.CredentialBlob.is_null() {
                        CredFree(p_credential as *const _);
                        return Err(InfrastructureError::SecretRetrieveError(format!(
                            "Credential '{}' has null blob pointer",
                            key
                        )));
                    }

                    let blob_slice = std::slice::from_raw_parts(
                        credential.CredentialBlob,
                        credential.CredentialBlobSize as usize,
                    );
                    let value = String::from_utf8(blob_slice.to_vec()).map_err(|e| {
                        InfrastructureError::SecretRetrieveError(format!(
                            "Invalid UTF-8 in credential '{}': {}",
                            key, e
                        ))
                    })?;

                    CredFree(p_credential as *const _);

                    Ok(Some(value))
                }
                Err(e) => {
                    if e.code() == ERROR_NOT_FOUND.to_hresult() {
                        Ok(None)
                    } else {
                        Err(InfrastructureError::SecretRetrieveError(format!(
                            "Failed to read credential '{}': {}",
                            key,
                            e.message()
                        )))
                    }
                }
            }
        }
    }

    fn delete(&self, key: &str) -> Result<(), InfrastructureError> {
        let target_name = super::format_key(key);
        let target_name_wide: Vec<u16> = target_name.encode_utf16().chain(Some(0)).collect();

        unsafe {
            match CredDeleteW(PCWSTR(target_name_wide.as_ptr()), CRED_TYPE_GENERIC, 0) {
                Ok(_) => Ok(()),
                Err(e) => {
                    if e.code() == ERROR_NOT_FOUND.to_hresult() {
                        Ok(())
                    } else {
                        Err(InfrastructureError::SecretDeleteError(format!(
                            "Failed to delete credential '{}': {}",
                            key,
                            e.message()
                        )))
                    }
                }
            }
        }
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    #[test]
    fn test_store_retrieve_roundtrip() {
        let manager = WindowsCredentialManager::new();
        let key = format!("test_roundtrip_{}", uuid::Uuid::new_v4());
        let value = "test_secret_value_123";

        manager.store(&key, value).unwrap();
        let retrieved = manager.retrieve(&key).unwrap();
        assert_eq!(retrieved, Some(value.to_string()));

        manager.delete(&key).unwrap();
    }

    #[test]
    fn test_delete_removes_secret() {
        let manager = WindowsCredentialManager::new();
        let key = format!("test_delete_{}", uuid::Uuid::new_v4());
        let value = "to_be_deleted";

        manager.store(&key, value).unwrap();
        manager.delete(&key).unwrap();

        let retrieved = manager.retrieve(&key).unwrap();
        assert_eq!(retrieved, None);
    }

    #[test]
    fn test_retrieve_nonexistent_returns_none() {
        let manager = WindowsCredentialManager::new();
        let key = format!("nonexistent_{}", uuid::Uuid::new_v4());

        let result = manager.retrieve(&key).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_namespace_isolation() {
        let manager = WindowsCredentialManager::new();
        let key = format!("namespace_test_{}", uuid::Uuid::new_v4());
        let value = "isolated_value";

        manager.store(&key, value).unwrap();

        let formatted = super::super::format_key(&key);
        assert!(formatted.starts_with("com.sapo.printer/"));

        manager.delete(&key).unwrap();
    }

    #[test]
    fn test_delete_nonexistent_succeeds() {
        let manager = WindowsCredentialManager::new();
        let key = format!("never_existed_{}", uuid::Uuid::new_v4());

        let result = manager.delete(&key);
        assert!(result.is_ok());
    }
}
