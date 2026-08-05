use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tokio::sync::{oneshot, Mutex};
use uuid::Uuid;

use crate::infrastructure::configs::db::connection::DbPool;
use crate::shared::errors::InfrastructureError;

pub const TOKEN_LIFETIME_SECS: i64 = 90 * 86400;
pub const PAIR_TIMEOUT_SECS: u64 = 60;

#[derive(Debug, Clone, Serialize)]
pub struct TokenResponse {
    pub api_token: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedOrigin {
    pub token_hash: String,
    pub origin: String,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
    pub expires_at: i64,
}

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
    Db(InfrastructureError),
    NoUiSubscriber,
}

impl std::fmt::Display for PairError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UserDenied => write!(f, "user denied"),
            Self::Timeout => write!(f, "user did not respond"),
            Self::Db(e) => write!(f, "db: {}", e),
            Self::NoUiSubscriber => write!(f, "no ui subscriber to receive pair request"),
        }
    }
}
impl std::error::Error for PairError {}

pub struct ApiTokenManager {
    db: DbPool,
    pending: Mutex<HashMap<Uuid, oneshot::Sender<bool>>>,
    ui_sink: Mutex<Option<tokio::sync::mpsc::UnboundedSender<PendingPairRequest>>>,
}

impl ApiTokenManager {
    pub fn new(db: DbPool) -> Arc<Self> {
        Arc::new(Self {
            db,
            pending: Mutex::new(HashMap::new()),
            ui_sink: Mutex::new(None),
        })
    }

    /// UI (Tauri) đăng ký nhận pair requests.
    pub async fn set_ui_sink(
        &self,
        sink: tokio::sync::mpsc::UnboundedSender<PendingPairRequest>,
    ) {
        *self.ui_sink.lock().await = Some(sink);
    }

    /// Request pairing. Blocks tới khi user approve/deny hoặc timeout.
    pub async fn request_pair(&self, origin: &str) -> Result<TokenResponse, PairError> {
        let request_id = Uuid::new_v4();
        let req = PendingPairRequest {
            request_id,
            origin: origin.to_string(),
            requested_at: SystemTime::now(),
        };
        let (tx, rx) = oneshot::channel::<bool>();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(request_id, tx);
        }

        {
            let sink = self.ui_sink.lock().await;
            if let Some(s) = sink.as_ref() {
                if s.send(req).is_err() {
                    return Err(PairError::NoUiSubscriber);
                }
            } else {
                return Err(PairError::NoUiSubscriber);
            }
        }

        let approved = match tokio::time::timeout(Duration::from_secs(PAIR_TIMEOUT_SECS), rx).await
        {
            Ok(Ok(v)) => v,
            Ok(Err(_)) => return Err(PairError::UserDenied),
            Err(_) => {
                self.pending.lock().await.remove(&request_id);
                return Err(PairError::Timeout);
            }
        };

        if !approved {
            return Err(PairError::UserDenied);
        }

        let (plaintext, response) = self
            .issue_token(origin)
            .await
            .map_err(PairError::Db)?;
        tracing::info!(origin, "Token issued");
        drop(plaintext); // shadow-clarify: response.api_token already carries it
        Ok(response)
    }

    /// UI resolve pair request. `approved=false` → deny.
    pub async fn resolve_pair(&self, request_id: Uuid, approved: bool) -> bool {
        let mut pending = self.pending.lock().await;
        if let Some(tx) = pending.remove(&request_id) {
            let _ = tx.send(approved);
            true
        } else {
            false
        }
    }

    async fn issue_token(
        &self,
        origin: &str,
    ) -> Result<(String, TokenResponse), InfrastructureError> {
        let mut token_bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut token_bytes);
        let plaintext = hex::encode(token_bytes);
        let salt = hex::encode({
            let mut s = [0u8; 16];
            rand::thread_rng().fill_bytes(&mut s);
            s
        });
        let hash = hash_token(&plaintext, &salt);
        let now = unix_now();
        let expires_at = now + TOKEN_LIFETIME_SECS;

        let conn = self.db.get().map_err(InfrastructureError::from)?;
        conn.execute(
            "INSERT INTO api_tokens (token_hash, token_salt, origin, created_at, expires_at, is_active) \
             VALUES (?1, ?2, ?3, ?4, ?5, 1)",
            rusqlite::params![hash, salt, origin, now, expires_at],
        )
            .map_err(|e| InfrastructureError::DatabaseError {
                reason: e.to_string(),
            })?;

        Ok((
            plaintext.clone(),
            TokenResponse {
                api_token: plaintext,
                expires_at,
            },
        ))
    }

    /// Verify token đã trả từ Authorization: Bearer. Trả origin nếu hợp lệ.
    /// Constant-time compare vs all active tokens (bounded by DB size).
    pub fn verify_token(&self, presented: &str) -> Option<String> {
        let conn = self.db.get().ok()?;
        let now = unix_now();
        let mut stmt = conn
            .prepare(
                "SELECT token_hash, token_salt, origin, expires_at \
                 FROM api_tokens WHERE is_active = 1 AND expires_at > ?1",
            )
            .ok()?;
        let rows = stmt
            .query_map(rusqlite::params![now], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .ok()?
            .collect::<Result<Vec<_>, _>>()
            .ok()?;

        let mut matched: Option<(String, String)> = None;
        for (hash, salt, origin, _exp) in rows {
            let candidate = hash_token(presented, &salt);
            if candidate.as_bytes().ct_eq(hash.as_bytes()).unwrap_u8() == 1 {
                matched = Some((hash, origin));
                // Do NOT break — continue to keep timing uniform.
            }
        }

        if let Some((hash, origin)) = matched {
            self.touch_last_used(&hash);
            Some(origin)
        } else {
            None
        }
    }

    fn touch_last_used(&self, token_hash: &str) {
        let Ok(conn) = self.db.get() else {
            return;
        };
        let now = unix_now();
        let new_exp = now + TOKEN_LIFETIME_SECS;
        let _ = conn.execute(
            "UPDATE api_tokens SET last_used_at = ?1, expires_at = ?2 WHERE token_hash = ?3",
            rusqlite::params![now, new_exp, token_hash],
        );
    }

    pub fn revoke(&self, token_hash: &str) -> Result<(), InfrastructureError> {
        let conn = self.db.get().map_err(InfrastructureError::from)?;
        conn.execute(
            "UPDATE api_tokens SET is_active = 0 WHERE token_hash = ?1",
            rusqlite::params![token_hash],
        )
            .map_err(|e| InfrastructureError::DatabaseError {
                reason: e.to_string(),
            })?;
        Ok(())
    }

    pub fn list_paired(&self) -> Result<Vec<PairedOrigin>, InfrastructureError> {
        let conn = self.db.get().map_err(InfrastructureError::from)?;
        let mut stmt = conn
            .prepare(
                "SELECT token_hash, origin, created_at, last_used_at, expires_at \
                 FROM api_tokens WHERE is_active = 1 ORDER BY created_at DESC",
            )
            .map_err(|e| InfrastructureError::DatabaseError {
                reason: e.to_string(),
            })?;
        let rows = stmt
            .query_map([], |row| {
                Ok(PairedOrigin {
                    token_hash: row.get(0)?,
                    origin: row.get(1)?,
                    created_at: row.get(2)?,
                    last_used_at: row.get(3)?,
                    expires_at: row.get(4)?,
                })
            })
            .map_err(|e| InfrastructureError::DatabaseError {
                reason: e.to_string(),
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| InfrastructureError::DatabaseError {
                reason: e.to_string(),
            })?;
        Ok(rows)
    }
}

fn hash_token(token: &str, salt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hasher.update(b"|");
    hasher.update(salt.as_bytes());
    hex::encode(hasher.finalize())
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::configs::db::migrations::run_migrations;

    fn setup() -> DbPool {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        let tid = std::thread::current().id();
        let tmp = std::env::temp_dir().join(format!("sapo_tokens_{nanos}_{tid:?}.db"));
        let pool = DbPool::new(tmp.to_str().unwrap()).unwrap();
        {
            let mut conn = pool.get().unwrap();
            run_migrations(&mut conn).unwrap();
        }
        pool
    }

    #[tokio::test]
    async fn issue_and_verify_roundtrip() {
        let pool = setup();
        let mgr = ApiTokenManager::new(pool);
        let (_plain, resp) = mgr.issue_token("https://a.mysapo.net").await.unwrap();
        let origin = mgr.verify_token(&resp.api_token).unwrap();
        assert_eq!(origin, "https://a.mysapo.net");
    }

    #[tokio::test]
    async fn invalid_token_rejected() {
        let pool = setup();
        let mgr = ApiTokenManager::new(pool);
        let _ = mgr.issue_token("https://a.mysapo.net").await.unwrap();
        assert!(mgr.verify_token("deadbeef").is_none());
    }

    #[tokio::test]
    async fn revoke_disables_token() {
        let pool = setup();
        let mgr = ApiTokenManager::new(pool);
        let (_p, resp) = mgr.issue_token("https://a.mysapo.net").await.unwrap();
        let list = mgr.list_paired().unwrap();
        let hash = list[0].token_hash.clone();
        mgr.revoke(&hash).unwrap();
        assert!(mgr.verify_token(&resp.api_token).is_none());
    }

    #[tokio::test]
    async fn expired_token_rejected() {
        let pool = setup();
        let mgr = ApiTokenManager::new(pool.clone());
        let (_p, resp) = mgr.issue_token("https://a.mysapo.net").await.unwrap();
        {
            let conn = pool.get().unwrap();
            conn.execute("UPDATE api_tokens SET expires_at = 1", [])
                .unwrap();
        }
        assert!(mgr.verify_token(&resp.api_token).is_none());
    }

    #[tokio::test]
    async fn verify_updates_sliding_expiry() {
        let pool = setup();
        let mgr = ApiTokenManager::new(pool);
        let (_p, resp) = mgr.issue_token("https://a.mysapo.net").await.unwrap();
        let before = mgr.list_paired().unwrap()[0].expires_at;
        std::thread::sleep(std::time::Duration::from_secs(1));
        let _ = mgr.verify_token(&resp.api_token).unwrap();
        let after = mgr.list_paired().unwrap()[0].expires_at;
        assert!(after >= before);
    }

    #[test]
    fn hash_stable_with_same_salt() {
        assert_eq!(hash_token("abc", "salt"), hash_token("abc", "salt"));
        assert_ne!(hash_token("abc", "salt1"), hash_token("abc", "salt2"));
    }
}
