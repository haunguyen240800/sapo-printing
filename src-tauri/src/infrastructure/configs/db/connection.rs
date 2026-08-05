use r2d2::{Pool, PooledConnection};
use r2d2_sqlite::SqliteConnectionManager;
use std::fmt;
use std::time::Duration;

#[derive(Debug)]
pub enum DatabaseError {
    ConnectionFailed { reason: String },
    MigrationFailed { reason: String },
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DatabaseError::ConnectionFailed { reason } => {
                write!(f, "DB connection failed: {reason}")
            }
            DatabaseError::MigrationFailed { reason } => {
                write!(f, "DB migration failed: {reason}")
            }
        }
    }
}

impl std::error::Error for DatabaseError {}

pub type SqliteConn = PooledConnection<SqliteConnectionManager>;

#[derive(Clone)]
pub struct DbPool(Pool<SqliteConnectionManager>);

impl DbPool {
    pub fn new(db_path: &str) -> Result<Self, DatabaseError> {
        let manager = SqliteConnectionManager::file(db_path).with_init(|conn| {
            conn.busy_timeout(Duration::from_secs(30))?;
            conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
            Ok(())
        });

        let pool = Pool::builder()
            .max_size(8)
            .build(manager)
            .map_err(|e| DatabaseError::ConnectionFailed {
                reason: e.to_string(),
            })?;

        Ok(Self(pool))
    }

    pub fn get(&self) -> Result<SqliteConn, DatabaseError> {
        self.0.get().map_err(|e| DatabaseError::ConnectionFailed {
            reason: e.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_pool_opens_wal_mode() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        let thread_id = std::thread::current().id();
        let tmp = std::env::temp_dir().join(format!("sapo_test_{nanos}_{thread_id:?}.db"));
        let pool = DbPool::new(tmp.to_str().unwrap()).unwrap();
        let mode: String = {
            let conn = pool.get().unwrap();
            conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))
                .unwrap()
        };
        drop(pool);
        let _ = std::fs::remove_file(&tmp);
        assert_eq!(mode, "wal");
    }
}
