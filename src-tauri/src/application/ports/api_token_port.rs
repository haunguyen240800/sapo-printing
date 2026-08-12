//! Port for API token management (device pairing + bearer-token auth).
//!
//! The interface layer (HTTP server, Tauri agent commands) depends on this
//! trait, not on the concrete `ApiTokenRepository` in infrastructure. Pairing
//! orchestration (notify UI â†’ await approval â†’ issue token) lives behind the
//! port so the interface never touches the database directly.

use std::time::SystemTime;

use async_trait::async_trait;
use uuid::Uuid;

/// A freshly issued bearer token and its absolute expiry (unix seconds).
#[derive(Debug, Clone)]
pub struct IssuedToken {
    pub api_token: String,
    pub expires_at: i64,
}

/// A previously paired origin, as surfaced to the UI.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PairedOrigin {
    pub token_hash: String,
    pub origin: String,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
    pub expires_at: i64,
}

/// A pending pairing request awaiting user approval, forwarded to the UI.
#[derive(Debug, Clone)]
pub struct PendingPairRequest {
    pub request_id: Uuid,
    pub origin: String,
    pub requested_at: SystemTime,
}

#[derive(Debug)]
pub enum PairError {
    UserDenied,
    Timeout,
    NoUiSubscriber,
    /// Backend failure (e.g. persistence error) while issuing the token.
    Backend(String),
}

impl std::fmt::Display for PairError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UserDenied => write!(f, "user denied"),
            Self::Timeout => write!(f, "user did not respond"),
            Self::NoUiSubscriber => write!(f, "no ui subscriber to receive pair request"),
            Self::Backend(msg) => write!(f, "backend: {}", msg),
        }
    }
}

impl std::error::Error for PairError {}

/// Sink the UI registers to receive pairing requests.
pub type PairRequestSink = tokio::sync::mpsc::UnboundedSender<PendingPairRequest>;

#[async_trait]
pub trait ApiTokenPort: Send + Sync {
    /// UI (Tauri) registers to receive pairing requests.
    async fn set_ui_sink(&self, sink: PairRequestSink);

    /// Request pairing for `origin`. Blocks until the user approves/denies or
    /// the request times out.
    async fn request_pair(&self, origin: &str) -> Result<IssuedToken, PairError>;

    /// UI resolves a pending pairing request. `approved = false` denies it.
    /// Returns whether a matching pending request existed.
    async fn resolve_pair(&self, request_id: Uuid, approved: bool) -> bool;

    /// Verify a presented bearer token; returns the paired origin if valid.
    fn verify_token(&self, presented: &str) -> Option<String>;

    /// List all currently active paired origins.
    fn list_paired(&self) -> Result<Vec<PairedOrigin>, String>;

    /// Revoke a token by its hash.
    fn revoke(&self, token_hash: &str) -> Result<(), String>;
}

