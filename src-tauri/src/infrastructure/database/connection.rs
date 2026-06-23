use rusqlite::Connection;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

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

#[derive(Clone)]
pub struct DbPool(Arc<Mutex<Connection>>);

impl DbPool {
    pub fn new(db_path: &str) -> Result<Self, DatabaseError> {
        let conn = Connection::open(db_path)
            .map_err(|e| DatabaseError::ConnectionFailed { reason: e.to_string() })?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .map_err(|e| DatabaseError::ConnectionFailed { reason: e.to_string() })?;
        Ok(Self(Arc::new(Mutex::new(conn))))
    }

    pub fn get(&self) -> MutexGuard<'_, Connection> {
        self.0.lock().expect("DB mutex poisoned")
    }

    /// Get the underlying Arc<Mutex<Connection>> for repository construction
    pub fn get_arc(&self) -> Arc<Mutex<Connection>> {
        self.0.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_pool_opens_wal_mode() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos();
        let thread_id = std::thread::current().id();
        let tmp = std::env::temp_dir().join(format!("sapo_test_{nanos}_{thread_id:?}.db"));
        let pool = DbPool::new(tmp.to_str().unwrap()).unwrap();
        let mode: String = {
            let conn = pool.get();
            conn.query_row("PRAGMA journal_mode", [], |row| row.get(0)).unwrap()
        };
        drop(pool); // ensure connection closed before deleting file (important on Windows)
        let _ = std::fs::remove_file(&tmp);
        assert_eq!(mode, "wal");
    }
}
