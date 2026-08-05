use std::time::SystemTime;

use super::cert_generator::CertBundle;

pub const SERVER_RENEW_THRESHOLD_DAYS: u64 = 30;
pub const CA_ROTATION_THRESHOLD_DAYS: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenewalStatus {
    Ok,
    RenewSoon,
    CaRotationNeeded,
    Expired,
}

pub fn needs_renewal(bundle: &CertBundle) -> RenewalStatus {
    let now = SystemTime::now();
    let server_remaining = bundle
        .server_expires_at
        .duration_since(now)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let ca_remaining = bundle
        .ca_expires_at
        .duration_since(now)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    if server_remaining == 0 || ca_remaining == 0 {
        return RenewalStatus::Expired;
    }
    if ca_remaining < CA_ROTATION_THRESHOLD_DAYS * 86400 {
        return RenewalStatus::CaRotationNeeded;
    }
    if server_remaining < SERVER_RENEW_THRESHOLD_DAYS * 86400 {
        return RenewalStatus::RenewSoon;
    }
    RenewalStatus::Ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn bundle(server_secs_left: u64, ca_secs_left: u64) -> CertBundle {
        let now = SystemTime::now();
        CertBundle {
            ca_cert_pem: String::new(),
            ca_key_pem: String::new(),
            server_cert_pem: String::new(),
            server_key_pem: String::new(),
            is_newly_generated: false,
            server_expires_at: now + Duration::from_secs(server_secs_left),
            ca_expires_at: now + Duration::from_secs(ca_secs_left),
        }
    }

    #[test]
    fn ok_when_both_far_from_expiry() {
        let b = bundle(200 * 86400, 3000 * 86400);
        assert_eq!(needs_renewal(&b), RenewalStatus::Ok);
    }

    #[test]
    fn renew_soon_when_server_under_30d() {
        let b = bundle(20 * 86400, 3000 * 86400);
        assert_eq!(needs_renewal(&b), RenewalStatus::RenewSoon);
    }

    #[test]
    fn ca_rotation_takes_priority() {
        let b = bundle(20 * 86400, 30 * 86400);
        assert_eq!(needs_renewal(&b), RenewalStatus::CaRotationNeeded);
    }

    #[test]
    fn expired_when_zero_remaining() {
        let now = SystemTime::now();
        let expired = CertBundle {
            ca_cert_pem: String::new(),
            ca_key_pem: String::new(),
            server_cert_pem: String::new(),
            server_key_pem: String::new(),
            is_newly_generated: false,
            server_expires_at: now - Duration::from_secs(3600),
            ca_expires_at: now + Duration::from_secs(3600 * 24 * 365),
        };
        assert_eq!(needs_renewal(&expired), RenewalStatus::Expired);
    }
}
