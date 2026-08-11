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

/// Thư mục chứa CA/server cert (và IPC state) dùng chung giữa cert-manager và app.
pub fn shared_cert_dir() -> PathBuf {
    if let Some(v) = std::env::var_os("SAPO_AGENT_DATA_DIR") {
        return PathBuf::from(v);
    }

    #[cfg(target_os = "windows")]
    {
        let program_data =
            std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into());
        PathBuf::from(program_data).join("SapoPrinter")
    }
    #[cfg(target_os = "macos")]
    {
        PathBuf::from("/Library/Application Support/SapoPrinter")
    }
    #[cfg(target_os = "linux")]
    {
        PathBuf::from("/var/lib/sapo-printer")
    }
}
