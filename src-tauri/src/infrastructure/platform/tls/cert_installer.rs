//! CA certificate installer — install/uninstall vào system trust store.
//!
//! **Elevated required**: chạy trong helper service (Windows Service/launchd/systemd)
//! hoặc installer script với admin privilege.
//!
//! - Windows: `LocalMachine\Root` (all users trust).
//! - macOS: `/Library/Keychains/System.keychain`.
//! - Linux: `/usr/local/share/ca-certificates/` + `update-ca-certificates`.

use std::path::Path;

use crate::infrastructure::errors::InfrastructureError;
#[cfg(any(target_os = "windows", target_os = "macos"))]
use crate::infrastructure::platform::agent_config::CA_FRIENDLY_NAME;
#[cfg(target_os = "linux")]
use crate::infrastructure::platform::agent_config::LINUX_CA_FILENAME;

pub trait CertInstaller: Send + Sync {
    /// Install CA vào system trust store. Idempotent — nếu đã tồn tại → replace.
    fn install_ca(&self, ca_pem_path: &Path) -> Result<(), InfrastructureError>;

    /// Uninstall CA khỏi system trust store. Idempotent — không lỗi nếu không tồn tại.
    fn uninstall_ca(&self) -> Result<(), InfrastructureError>;

    /// Verify CA đã trusted trong system store.
    fn is_ca_trusted(&self) -> bool;
}

#[cfg(target_os = "windows")]
pub use windows_impl::WindowsCertInstaller as PlatformInstaller;

#[cfg(target_os = "macos")]
pub use macos_impl::MacosCertInstaller as PlatformInstaller;

#[cfg(target_os = "linux")]
pub use linux_impl::LinuxCertInstaller as PlatformInstaller;

// ===================== Windows =====================

#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use std::process::Command;

    pub struct WindowsCertInstaller;

    impl WindowsCertInstaller {
        pub fn new() -> Self {
            Self
        }
    }

    impl Default for WindowsCertInstaller {
        fn default() -> Self {
            Self::new()
        }
    }

    impl CertInstaller for WindowsCertInstaller {
        fn install_ca(&self, ca_pem_path: &Path) -> Result<(), InfrastructureError> {
            // certutil -addstore -f "Root" <ca.pem>
            // -f: overwrite existing entry with same subject.
            // Cần admin/SYSTEM khi target LocalMachine store.
            let output = Command::new("certutil")
                .args(["-addstore", "-f", "Root"])
                .arg(ca_pem_path)
                .output()
                .map_err(|e| {
                    InfrastructureError::TlsError(format!("certutil spawn failed: {}", e))
                })?;
            if !output.status.success() {
                return Err(InfrastructureError::TlsError(format!(
                    "certutil addstore failed (exit {}): {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr)
                )));
            }
            Ok(())
        }

        fn uninstall_ca(&self) -> Result<(), InfrastructureError> {
            // certutil -delstore Root "Sapo Printer Local CA"
            let output = Command::new("certutil")
                .args(["-delstore", "Root", CA_FRIENDLY_NAME])
                .output()
                .map_err(|e| {
                    InfrastructureError::TlsError(format!("certutil spawn failed: {}", e))
                })?;
            // exit 0 = deleted, exit non-zero + "Cannot find" = idempotent OK.
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                let combined = format!("{}\n{}", stdout, stderr).to_lowercase();
                if combined.contains("cannot find")
                    || combined.contains("not found")
                    || combined.contains("element not found")
                {
                    return Ok(());
                }
                return Err(InfrastructureError::TlsError(format!(
                    "certutil delstore failed (exit {}): {}",
                    output.status, stderr
                )));
            }
            Ok(())
        }

        fn is_ca_trusted(&self) -> bool {
            // certutil -store Root "Sapo Printer Local CA" → exit 0 nếu tìm thấy.
            Command::new("certutil")
                .args(["-store", "Root", CA_FRIENDLY_NAME])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
    }
}

// ===================== macOS =====================

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::*;
    use std::process::Command;

    const SYSTEM_KEYCHAIN: &str = "/Library/Keychains/System.keychain";

    pub struct MacosCertInstaller;

    impl MacosCertInstaller {
        pub fn new() -> Self {
            Self
        }
    }

    impl Default for MacosCertInstaller {
        fn default() -> Self {
            Self::new()
        }
    }

    impl CertInstaller for MacosCertInstaller {
        fn install_ca(&self, ca_pem_path: &Path) -> Result<(), InfrastructureError> {
            // security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain <ca.pem>
            // -d: add to admin cert store (system).
            // -r trustRoot: trust as root CA.
            // Cần root khi ghi System.keychain.
            let output = Command::new("security")
                .args([
                    "add-trusted-cert",
                    "-d",
                    "-r",
                    "trustRoot",
                    "-k",
                    SYSTEM_KEYCHAIN,
                ])
                .arg(ca_pem_path)
                .output()
                .map_err(|e| {
                    InfrastructureError::TlsError(format!("security spawn failed: {}", e))
                })?;
            if !output.status.success() {
                return Err(InfrastructureError::TlsError(format!(
                    "security add-trusted-cert failed (exit {}): {}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr)
                )));
            }
            Ok(())
        }

        fn uninstall_ca(&self) -> Result<(), InfrastructureError> {
            // security delete-certificate -c "Sapo Printer Local CA" /Library/Keychains/System.keychain
            let output = Command::new("security")
                .args([
                    "delete-certificate",
                    "-c",
                    CA_FRIENDLY_NAME,
                    SYSTEM_KEYCHAIN,
                ])
                .output()
                .map_err(|e| {
                    InfrastructureError::TlsError(format!("security spawn failed: {}", e))
                })?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
                if stderr.contains("could not be found") || stderr.contains("no matching") {
                    return Ok(());
                }
                return Err(InfrastructureError::TlsError(format!(
                    "security delete-certificate failed (exit {}): {}",
                    output.status, stderr
                )));
            }
            Ok(())
        }

        fn is_ca_trusted(&self) -> bool {
            // security find-certificate -c "Sapo Printer Local CA" /Library/Keychains/System.keychain
            Command::new("security")
                .args(["find-certificate", "-c", CA_FRIENDLY_NAME, SYSTEM_KEYCHAIN])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
    }
}

// ===================== Linux =====================

#[cfg(target_os = "linux")]
mod linux_impl {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    const CA_TRUST_DIR: &str = "/usr/local/share/ca-certificates";

    pub struct LinuxCertInstaller;

    impl LinuxCertInstaller {
        pub fn new() -> Self {
            Self
        }

        fn target_path() -> PathBuf {
            PathBuf::from(CA_TRUST_DIR).join(LINUX_CA_FILENAME)
        }
    }

    impl Default for LinuxCertInstaller {
        fn default() -> Self {
            Self::new()
        }
    }

    impl CertInstaller for LinuxCertInstaller {
        fn install_ca(&self, ca_pem_path: &Path) -> Result<(), InfrastructureError> {
            fs::create_dir_all(CA_TRUST_DIR).map_err(|e| {
                InfrastructureError::TlsError(format!("mkdir {}: {}", CA_TRUST_DIR, e))
            })?;
            fs::copy(ca_pem_path, Self::target_path()).map_err(|e| {
                InfrastructureError::TlsError(format!("copy ca to trust dir: {}", e))
            })?;

            // Debian/Ubuntu + RHEL family both accept update-ca-certificates on modern distros.
            // RHEL uses update-ca-trust; try both.
            let update = Command::new("update-ca-certificates").output();
            if let Ok(out) = update {
                if out.status.success() {
                    return Ok(());
                }
            }
            let update = Command::new("update-ca-trust").arg("extract").output();
            if let Ok(out) = update {
                if out.status.success() {
                    return Ok(());
                }
                return Err(InfrastructureError::TlsError(format!(
                    "update-ca-trust failed (exit {}): {}",
                    out.status,
                    String::from_utf8_lossy(&out.stderr)
                )));
            }
            Err(InfrastructureError::TlsError(
                "Neither update-ca-certificates nor update-ca-trust available".into(),
            ))
        }

        fn uninstall_ca(&self) -> Result<(), InfrastructureError> {
            let target = Self::target_path();
            if target.exists() {
                fs::remove_file(&target).map_err(|e| {
                    InfrastructureError::TlsError(format!("remove {}: {}", target.display(), e))
                })?;
            }
            // Refresh trust store — không fail nếu tool không có (đã xóa file rồi).
            let _ = Command::new("update-ca-certificates")
                .arg("--fresh")
                .output();
            let _ = Command::new("update-ca-trust").arg("extract").output();
            Ok(())
        }

        fn is_ca_trusted(&self) -> bool {
            Self::target_path().exists()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_installer_constructible() {
        let _installer = PlatformInstaller::default();
    }

    // Real install/uninstall tests đòi hỏi elevated + system state mutation,
    // được test qua integration test trong Sprint 3 (helper service e2e)
    // và manual verify trên máy target.
}
