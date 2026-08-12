//! Self-signed CA + server certificate generator.
//!
//! # Design
//! - CA cert: 10 năm, `CN=Sapo Printer Local CA`. `keyCertSign`, `cRLSign`.
//! - Server cert: 397 ngày (giới hạn browser), `CN=local.mysapo.net`,
//!   SAN: `local.mysapo.net`, `localhost`, `127.0.0.1`, `::1`.
//!
//! # Layout on disk
//! `<data_dir>/tls/{ca.pem, ca.key, server.pem, server.key}`
//!
//! CA key phải được helper service bảo vệ (mode 0600, owner SYSTEM/root)
//! ở deployment. Ở test/dev, mode restriction áp dụng best-effort.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use ::time::Duration as TimeDuration;
use ::time::OffsetDateTime;
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, KeyUsagePurpose,
    SanType,
};
use x509_parser::prelude::*;

use crate::infrastructure::errors::InfrastructureError;

const CA_VALIDITY_DAYS: i64 = 3650;
const SERVER_VALIDITY_DAYS: i64 = 397;
const CA_COMMON_NAME: &str = "Sapo Printer Local CA";
const SERVER_COMMON_NAME: &str = "local.mysapo.net";

pub struct CertPaths {
    pub ca_pem: PathBuf,
    pub ca_key: PathBuf,
    pub server_pem: PathBuf,
    pub server_key: PathBuf,
}

impl CertPaths {
    pub fn under(data_dir: &Path) -> Self {
        let tls_dir = data_dir.join("tls");
        Self {
            ca_pem: tls_dir.join("ca.pem"),
            ca_key: tls_dir.join("ca.key"),
            server_pem: tls_dir.join("server.pem"),
            server_key: tls_dir.join("server.key"),
        }
    }

    pub fn tls_dir(&self) -> &Path {
        self.ca_pem.parent().expect("ca.pem must have parent")
    }
}

#[derive(Debug, Clone)]
pub struct CertBundle {
    pub ca_cert_pem: String,
    pub ca_key_pem: String,
    pub server_cert_pem: String,
    pub server_key_pem: String,
    pub is_newly_generated: bool,
    pub ca_expires_at: SystemTime,
    pub server_expires_at: SystemTime,
}

pub struct CertGenerator;

impl CertGenerator {
    /// Idempotent: nếu file tồn tại và còn valid → load; nếu không → sinh mới.
    pub fn load_or_generate(data_dir: &Path) -> Result<CertBundle, InfrastructureError> {
        let paths = CertPaths::under(data_dir);
        fs::create_dir_all(paths.tls_dir())
            .map_err(|e| InfrastructureError::TlsError(format!("Cannot create tls dir: {}", e)))?;

        if paths_all_exist(&paths) {
            match Self::load_from_disk(&paths) {
                Ok(bundle) => return Ok(bundle),
                Err(e) => {
                    tracing::warn!("Existing cert unreadable ({}), regenerating", e);
                }
            }
        }

        let ca = Self::generate_ca()?;
        let server = Self::generate_server_signed_by(&ca)?;

        let bundle = CertBundle {
            ca_cert_pem: ca.cert_pem.clone(),
            ca_key_pem: ca.key_pem.clone(),
            server_cert_pem: server.cert_pem.clone(),
            server_key_pem: server.key_pem.clone(),
            is_newly_generated: true,
            ca_expires_at: parse_not_after(&ca.cert_pem)?,
            server_expires_at: parse_not_after(&server.cert_pem)?,
        };

        write_secure(&paths.ca_pem, &bundle.ca_cert_pem, 0o644)?;
        write_secure(&paths.ca_key, &bundle.ca_key_pem, 0o600)?;
        write_secure(&paths.server_pem, &bundle.server_cert_pem, 0o644)?;
        write_secure(&paths.server_key, &bundle.server_key_pem, 0o640)?;

        Ok(bundle)
    }

    /// Regenerate server cert bằng CA hiện tại. CA key phải đọc được.
    pub fn renew_server_cert(data_dir: &Path) -> Result<CertBundle, InfrastructureError> {
        let paths = CertPaths::under(data_dir);
        let ca_cert_pem = fs::read_to_string(&paths.ca_pem)
            .map_err(|e| InfrastructureError::TlsError(format!("Read ca.pem: {}", e)))?;
        let ca_key_pem = fs::read_to_string(&paths.ca_key)
            .map_err(|e| InfrastructureError::TlsError(format!("Read ca.key: {}", e)))?;

        let ca = CaMaterial {
            cert_pem: ca_cert_pem,
            key_pem: ca_key_pem,
        };
        let server = Self::generate_server_signed_by(&ca)?;

        write_secure(&paths.server_pem, &server.cert_pem, 0o644)?;
        write_secure(&paths.server_key, &server.key_pem, 0o640)?;

        Ok(CertBundle {
            ca_cert_pem: ca.cert_pem.clone(),
            ca_key_pem: ca.key_pem.clone(),
            server_cert_pem: server.cert_pem.clone(),
            server_key_pem: server.key_pem.clone(),
            is_newly_generated: false,
            ca_expires_at: parse_not_after(&ca.cert_pem)?,
            server_expires_at: parse_not_after(&server.cert_pem)?,
        })
    }

    fn generate_ca() -> Result<CaMaterial, InfrastructureError> {
        let mut params = CertificateParams::default();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, CA_COMMON_NAME);
        dn.push(DnType::OrganizationName, "Sapo");
        params.distinguished_name = dn;
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
            KeyUsagePurpose::DigitalSignature,
        ];
        let now = OffsetDateTime::now_utc();
        params.not_before = now;
        params.not_after = now + TimeDuration::days(CA_VALIDITY_DAYS);

        let key = KeyPair::generate()
            .map_err(|e| InfrastructureError::TlsError(format!("CA keypair: {}", e)))?;
        let cert = params
            .self_signed(&key)
            .map_err(|e| InfrastructureError::TlsError(format!("CA self-sign: {}", e)))?;

        Ok(CaMaterial {
            cert_pem: cert.pem(),
            key_pem: key.serialize_pem(),
        })
    }

    fn generate_server_signed_by(ca: &CaMaterial) -> Result<CertMaterial, InfrastructureError> {
        let ca_key = KeyPair::from_pem(&ca.key_pem)
            .map_err(|e| InfrastructureError::TlsError(format!("Parse CA key: {}", e)))?;
        let ca_params = CertificateParams::from_ca_cert_pem(&ca.cert_pem)
            .map_err(|e| InfrastructureError::TlsError(format!("Parse CA cert params: {}", e)))?;
        let ca_cert = ca_params
            .self_signed(&ca_key)
            .map_err(|e| InfrastructureError::TlsError(format!("Rebuild CA cert: {}", e)))?;

        let mut params = CertificateParams::default();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, SERVER_COMMON_NAME);
        dn.push(DnType::OrganizationName, "Sapo");
        params.distinguished_name = dn;
        params.subject_alt_names = vec![
            SanType::DnsName(SERVER_COMMON_NAME.try_into().unwrap()),
            SanType::DnsName("localhost".try_into().unwrap()),
            SanType::IpAddress("127.0.0.1".parse().unwrap()),
            SanType::IpAddress("::1".parse().unwrap()),
        ];
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyEncipherment,
        ];
        params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
        let now = OffsetDateTime::now_utc();
        params.not_before = now;
        params.not_after = now + TimeDuration::days(SERVER_VALIDITY_DAYS);

        let server_key = KeyPair::generate()
            .map_err(|e| InfrastructureError::TlsError(format!("Server keypair: {}", e)))?;
        let cert = params
            .signed_by(&server_key, &ca_cert, &ca_key)
            .map_err(|e| InfrastructureError::TlsError(format!("Server sign: {}", e)))?;

        Ok(CertMaterial {
            cert_pem: cert.pem(),
            key_pem: server_key.serialize_pem(),
        })
    }

    fn load_from_disk(paths: &CertPaths) -> Result<CertBundle, InfrastructureError> {
        let ca_cert_pem = fs::read_to_string(&paths.ca_pem)?;
        let ca_key_pem = fs::read_to_string(&paths.ca_key)?;
        let server_cert_pem = fs::read_to_string(&paths.server_pem)?;
        let server_key_pem = fs::read_to_string(&paths.server_key)?;

        Ok(CertBundle {
            ca_expires_at: parse_not_after(&ca_cert_pem)?,
            server_expires_at: parse_not_after(&server_cert_pem)?,
            ca_cert_pem,
            ca_key_pem,
            server_cert_pem,
            server_key_pem,
            is_newly_generated: false,
        })
    }
}

struct CaMaterial {
    cert_pem: String,
    key_pem: String,
}

struct CertMaterial {
    cert_pem: String,
    key_pem: String,
}

fn paths_all_exist(paths: &CertPaths) -> bool {
    paths.ca_pem.exists()
        && paths.ca_key.exists()
        && paths.server_pem.exists()
        && paths.server_key.exists()
}

fn parse_not_after(pem: &str) -> Result<SystemTime, InfrastructureError> {
    let (_, pem_block) = parse_x509_pem(pem.as_bytes())
        .map_err(|e| InfrastructureError::TlsError(format!("PEM parse: {}", e)))?;
    let (_, cert) = X509Certificate::from_der(&pem_block.contents)
        .map_err(|e| InfrastructureError::TlsError(format!("DER parse: {}", e)))?;
    let ts = cert.validity().not_after.timestamp();
    Ok(SystemTime::UNIX_EPOCH + Duration::from_secs(ts as u64))
}

#[cfg(unix)]
fn write_secure(path: &Path, contents: &str, mode: u32) -> Result<(), InfrastructureError> {
    use std::os::unix::fs::PermissionsExt;
    fs::write(path, contents)?;
    let mut perm = fs::metadata(path)?.permissions();
    perm.set_mode(mode);
    fs::set_permissions(path, perm)?;
    Ok(())
}

#[cfg(not(unix))]
fn write_secure(path: &Path, contents: &str, _mode: u32) -> Result<(), InfrastructureError> {
    fs::write(path, contents)?;
    // Windows ACL: sẽ được installer/helper service set. Runtime chỉ ghi.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};
    use tempfile::TempDir;

    #[test]
    fn generates_new_bundle_when_missing() {
        let tmp = TempDir::new().unwrap();
        let bundle = CertGenerator::load_or_generate(tmp.path()).unwrap();
        assert!(bundle.is_newly_generated);
        assert!(bundle.ca_cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(bundle.server_cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(bundle.ca_key_pem.contains("PRIVATE KEY"));
        assert!(bundle.server_key_pem.contains("PRIVATE KEY"));
    }

    #[test]
    fn ca_valid_for_ten_years() {
        let tmp = TempDir::new().unwrap();
        let bundle = CertGenerator::load_or_generate(tmp.path()).unwrap();
        let elapsed = bundle
            .ca_expires_at
            .duration_since(SystemTime::now())
            .unwrap();
        // Cho phép sai số 1 ngày
        assert!(elapsed >= Duration::from_secs((CA_VALIDITY_DAYS - 1) as u64 * 86400));
        assert!(elapsed <= Duration::from_secs((CA_VALIDITY_DAYS + 1) as u64 * 86400));
    }

    #[test]
    fn server_valid_397_days() {
        let tmp = TempDir::new().unwrap();
        let bundle = CertGenerator::load_or_generate(tmp.path()).unwrap();
        let elapsed = bundle
            .server_expires_at
            .duration_since(SystemTime::now())
            .unwrap();
        assert!(elapsed <= Duration::from_secs(398 * 86400));
        assert!(elapsed >= Duration::from_secs(395 * 86400));
    }

    #[test]
    fn load_when_exists_does_not_regenerate() {
        let tmp = TempDir::new().unwrap();
        let first = CertGenerator::load_or_generate(tmp.path()).unwrap();
        let second = CertGenerator::load_or_generate(tmp.path()).unwrap();
        assert!(!second.is_newly_generated);
        assert_eq!(first.ca_cert_pem, second.ca_cert_pem);
        assert_eq!(first.server_cert_pem, second.server_cert_pem);
    }

    #[test]
    fn renew_replaces_server_cert_only() {
        let tmp = TempDir::new().unwrap();
        let first = CertGenerator::load_or_generate(tmp.path()).unwrap();
        std::thread::sleep(Duration::from_millis(1100));
        let renewed = CertGenerator::renew_server_cert(tmp.path()).unwrap();
        assert_eq!(first.ca_cert_pem, renewed.ca_cert_pem);
        assert_ne!(first.server_cert_pem, renewed.server_cert_pem);
    }

    #[test]
    fn server_cert_contains_san_localhost_and_local_mysapo_net() {
        let tmp = TempDir::new().unwrap();
        let bundle = CertGenerator::load_or_generate(tmp.path()).unwrap();
        let (_, pem) = parse_x509_pem(bundle.server_cert_pem.as_bytes()).unwrap();
        let (_, cert) = X509Certificate::from_der(&pem.contents).unwrap();
        let ext = cert
            .subject_alternative_name()
            .unwrap()
            .unwrap()
            .value
            .general_names
            .clone();
        let dns: Vec<_> = ext
            .iter()
            .filter_map(|n| match n {
                GeneralName::DNSName(s) => Some(*s),
                _ => None,
            })
            .collect();
        assert!(dns.contains(&"local.mysapo.net"));
        assert!(dns.contains(&"localhost"));
    }
}
