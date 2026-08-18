//! Shared resolver cho thư mục chứa cert TLS (CA + server) và IPC state.
//!
//! Cert-manager (chạy elevated bởi installer) và main app PHẢI đồng ý về cùng
//! một thư mục, nếu không app sẽ tự sinh CA riêng và phục vụ cert không được
//! tin cậy (xem sự cố data_dir mismatch). Đây là single source of truth.
//!
//! Thứ tự ưu tiên:
//! 1. Env `SAPO_AGENT_DATA_DIR` (dùng cho dev / test / override).
//! 2. Default theo nền tảng (machine-wide, helper elevated ghi được):
//!    - Windows: `%ProgramData%\SapoPrinter`
//!    - macOS:   `/Library/Application Support/SapoPrinter`
//!    - Linux:   `/var/lib/sapo-printer`

use std::path::PathBuf;

use crate::infrastructure::platform::agent_config::DATA_DIR_ENV;

#[cfg(target_os = "linux")]
use crate::infrastructure::platform::agent_config::LINUX_DATA_DIR;
#[cfg(target_os = "macos")]
use crate::infrastructure::platform::agent_config::MACOS_DATA_DIR;
#[cfg(target_os = "windows")]
use crate::infrastructure::platform::agent_config::WINDOWS_DATA_DIR_NAME;

/// Thư mục chứa CA/server cert (và IPC state) dùng chung giữa cert-manager và app.
pub fn shared_cert_dir() -> PathBuf {
    if let Some(v) = std::env::var_os(DATA_DIR_ENV) {
        return PathBuf::from(v);
    }

    #[cfg(target_os = "windows")]
    {
        let program_data =
            std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into());
        PathBuf::from(program_data).join(WINDOWS_DATA_DIR_NAME)
    }
    #[cfg(target_os = "macos")]
    {
        PathBuf::from(MACOS_DATA_DIR)
    }
    #[cfg(target_os = "linux")]
    {
        PathBuf::from(LINUX_DATA_DIR)
    }
}
