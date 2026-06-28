//! Application services — coordination helpers used by multiple use cases.

pub mod audit_service;
pub mod secret_key_service;

pub use audit_service::{
    cleanup_old_events, get_audit_trail, verify_audit_trail_integrity, verify_event_integrity,
    AuditIntegrityReport,
};
pub use secret_key_service::{format_key, validate_key, MAX_SECRET_SIZE};
