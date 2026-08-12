use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResult {
    pub update_available: bool,
    pub version: Option<String>,
    pub release_notes: Option<String>,
}

/// Guard to prevent concurrent install operations.
/// Use Arc<AtomicBool> shared between command handlers.
pub struct InstallGuard {
    installing: Arc<AtomicBool>,
}

/// RAII guard that releases InstallGuard on drop (panic-safe).
pub struct InstallGuardGuard<'a> {
    guard: &'a InstallGuard,
}

impl InstallGuard {
    pub fn new() -> Self {
        Self {
            installing: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Try to acquire the install lock. Returns Some(guard) if acquired, None if already installing.
    /// The returned guard will release the lock when dropped (panic-safe).
    pub fn try_acquire(&self) -> Option<InstallGuardGuard<'_>> {
        if self
            .installing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            Some(InstallGuardGuard { guard: self })
        } else {
            None
        }
    }

    fn release(&self) {
        self.installing.store(false, Ordering::SeqCst);
    }
}

impl<'a> Drop for InstallGuardGuard<'a> {
    fn drop(&mut self) {
        self.guard.release();
    }
}

impl Default for InstallGuard {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn check_for_updates(app: &AppHandle) -> Result<UpdateCheckResult, String> {
    let updater = app
        .updater()
        .map_err(|e| format!("Updater init failed: {}", e))?;
    match updater.check().await {
        Ok(Some(update)) => Ok(UpdateCheckResult {
            update_available: true,
            version: Some(update.version.clone()),
            release_notes: update.body.clone(),
        }),
        Ok(None) => Ok(UpdateCheckResult {
            update_available: false,
            version: None,
            release_notes: None,
        }),
        Err(e) => Err(format!("Update check failed: {}", e)),
    }
}

pub async fn download_and_install_update(app: &AppHandle) -> Result<(), String> {
    let updater = app
        .updater()
        .map_err(|e| format!("Updater init failed: {}", e))?;
    let update = updater
        .check()
        .await
        .map_err(|e| format!("Update check failed: {}", e))?
        .ok_or_else(|| "No update available".to_string())?;

    update
        .download_and_install(
            |chunk_length, content_length| {
                tracing::info!(
                    target = "sapo_printer::updater",
                    chunk_length,
                    content_length = ?content_length,
                    "Downloading update..."
                );
            },
            || {
                tracing::info!(target = "sapo_printer::updater", "Update download finished");
            },
        )
        .await
        .map_err(|e| format!("Update install failed: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_check_result_serialize_with_update() {
        let result = UpdateCheckResult {
            update_available: true,
            version: Some("0.2.0".to_string()),
            release_notes: Some("Bug fixes".to_string()),
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["update_available"], true);
        assert_eq!(json["version"], "0.2.0");
        assert_eq!(json["release_notes"], "Bug fixes");
    }

    #[test]
    fn test_update_check_result_serialize_no_update() {
        let result = UpdateCheckResult {
            update_available: false,
            version: None,
            release_notes: None,
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["update_available"], false);
        assert!(json["version"].is_null());
        assert!(json["release_notes"].is_null());
    }

    #[test]
    fn test_update_check_result_deserialize() {
        let json = r#"{"update_available":true,"version":"1.0.0","release_notes":"New features"}"#;
        let result: UpdateCheckResult = serde_json::from_str(json).unwrap();
        assert!(result.update_available);
        assert_eq!(result.version.as_deref(), Some("1.0.0"));
        assert_eq!(result.release_notes.as_deref(), Some("New features"));
    }

    #[test]
    fn test_update_check_result_roundtrip() {
        let original = UpdateCheckResult {
            update_available: true,
            version: Some("0.9.9".to_string()),
            release_notes: None,
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: UpdateCheckResult = serde_json::from_str(&json).unwrap();
        assert_eq!(original.update_available, deserialized.update_available);
        assert_eq!(original.version, deserialized.version);
        assert_eq!(original.release_notes, deserialized.release_notes);
    }

    #[test]
    fn test_error_message_formatting() {
        let err = format!("Updater init failed: {}", "plugin not registered");
        assert_eq!(err, "Updater init failed: plugin not registered");
        assert!(!err.contains("panic"));
    }
}
