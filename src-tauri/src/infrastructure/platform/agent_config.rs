//! Canonical constants owned by the privileged agent.
//!
//! Tauri-owned metadata and updater settings are intentionally absent here;
//! `build.rs` derives those values from `tauri.conf.json` at compile time.

/// Windows Service identity.
pub const SERVICE_NAME: &str = "SapoPrinterAgent";

/// Installed executable names.
pub const APP_EXE_NAME: &str = "sapo-printer.exe";
pub const AGENT_EXE_NAME: &str = "sapo-printer-cert-manager.exe";

/// Platform entry selected from Tauri's `latest.json` update manifest.
pub const UPDATER_PLATFORM_KEY: &str = "windows-x86_64";

/// IPC endpoints used by the app and privileged agent.
#[cfg(target_os = "windows")]
pub const IPC_ENDPOINT: &str = r"\\.\pipe\sapo-printer-agent";

#[cfg(unix)]
pub const IPC_ENDPOINT: &str = "/var/run/sapo-printer-agent.sock";

/// Shared machine-level storage configuration.
pub const DATA_DIR_ENV: &str = "SAPO_AGENT_DATA_DIR";
#[cfg(target_os = "windows")]
pub const WINDOWS_DATA_DIR_NAME: &str = "SapoPrinter";
#[cfg(target_os = "macos")]
pub const MACOS_DATA_DIR: &str = "/Library/Application Support/SapoPrinter";
#[cfg(target_os = "linux")]
pub const LINUX_DATA_DIR: &str = "/var/lib/sapo-printer";

/// Certificate identity and trust-store metadata.
pub const CA_COMMON_NAME: &str = "Sapo Printer Local CA";
pub const CA_FRIENDLY_NAME: &str = CA_COMMON_NAME;
pub const CERT_ORGANIZATION_NAME: &str = "Sapo";
pub const SERVER_COMMON_NAME: &str = "local.mysapo.net";
pub const LINUX_CA_FILENAME: &str = "sapo-printer-ca.crt";
