use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};

use super::connection::DatabaseError;

const MIGRATION_1: &str = "
CREATE TABLE app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
INSERT INTO app_settings (key, value) VALUES
    ('temp_file_retention_hours', '24');

CREATE TABLE print_jobs (
    id TEXT PRIMARY KEY CHECK(length(id) = 36),
    printer_name TEXT NOT NULL,
    document_url TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'PENDING',
    retry_count INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    completed_at INTEGER,
    scheduled_at INTEGER,
    error_message TEXT,
    output_path TEXT,
    settings_json TEXT
);
CREATE INDEX idx_print_jobs_status ON print_jobs(status);
CREATE INDEX idx_print_jobs_created_at ON print_jobs(created_at);
CREATE INDEX idx_print_jobs_scheduled ON print_jobs(scheduled_at);

CREATE TABLE events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    aggregate_id TEXT NOT NULL,
    sequence_number INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    UNIQUE(aggregate_id, sequence_number)
);
CREATE INDEX idx_events_aggregate ON events(aggregate_id);
CREATE INDEX idx_events_type ON events(event_type);

CREATE TABLE api_tokens (
    token_hash TEXT PRIMARY KEY,
    token_salt TEXT NOT NULL,
    origin TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    last_used_at INTEGER,
    expires_at INTEGER NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_tokens_expires ON api_tokens(expires_at);
";

pub fn run_migrations(conn: &mut Connection) -> Result<(), DatabaseError> {
    let migrations = Migrations::new(vec![M::up(MIGRATION_1)]);
    migrations
        .to_latest(conn)
        .map_err(|e| DatabaseError::MigrationFailed {
            reason: e.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_test_conn() -> Connection {
        Connection::open_in_memory().unwrap()
    }

    #[test]
    fn test_migrations_create_runtime_tables_only() {
        let mut conn = open_test_conn();
        run_migrations(&mut conn).unwrap();

        let mut statement = conn
            .prepare(
                "SELECT name FROM sqlite_master \
                 WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
            )
            .unwrap();
        let tables: Vec<String> = statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(
            tables,
            ["api_tokens", "app_settings", "events", "print_jobs"]
        );
    }

    #[test]
    fn test_migrations_seed_only_used_app_setting() {
        let mut conn = open_test_conn();
        run_migrations(&mut conn).unwrap();

        let setting: (String, String) = conn
            .query_row("SELECT key, value FROM app_settings", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();

        assert_eq!(setting, ("temp_file_retention_hours".into(), "24".into()));
    }

    #[test]
    fn test_migrations_idempotent() {
        let mut conn = open_test_conn();
        run_migrations(&mut conn).unwrap();
        run_migrations(&mut conn).unwrap();
    }

    #[test]
    fn test_print_jobs_schema_constraints() {
        let mut conn = open_test_conn();
        run_migrations(&mut conn).unwrap();
        let now = 1_700_000_000i64;
        let job_id = "11111111-1111-1111-1111-111111111111";

        conn.execute(
            "INSERT INTO print_jobs (id, printer_name, document_url, status, created_at, updated_at)
             VALUES (?1, 'HP LaserJet', 'https://s3.example.com/doc.pdf', 'PENDING', ?2, ?2)",
            rusqlite::params![job_id, now],
        )
        .unwrap();

        let (retry_count, completed_at): (i64, Option<i64>) = conn
            .query_row(
                "SELECT retry_count, completed_at FROM print_jobs WHERE id = ?1",
                rusqlite::params![job_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(retry_count, 0);
        assert!(completed_at.is_none());

        let result = conn.execute(
            "INSERT INTO print_jobs (id, printer_name, document_url, status, created_at, updated_at)
             VALUES (?1, 'Another Printer', 'https://s3.example.com/doc2.pdf', 'PENDING', ?2, ?2)",
            rusqlite::params![job_id, now],
        );
        assert!(result.is_err(), "Duplicate primary key must fail");
    }

    #[test]
    fn test_events_unique_aggregate_sequence() {
        let mut conn = open_test_conn();
        run_migrations(&mut conn).unwrap();
        let now = 1_700_000_000i64;

        conn.execute(
            "INSERT INTO events (aggregate_id, sequence_number, event_type, payload, timestamp)
             VALUES ('agg-1', 1, 'PrintJobCreated', '{}', ?1)",
            rusqlite::params![now],
        )
        .unwrap();

        let result = conn.execute(
            "INSERT INTO events (aggregate_id, sequence_number, event_type, payload, timestamp)
             VALUES ('agg-1', 1, 'PrintJobQueued', '{}', ?1)",
            rusqlite::params![now],
        );
        assert!(
            result.is_err(),
            "Duplicate (aggregate_id, sequence_number) must fail"
        );

        conn.execute(
            "INSERT INTO events (aggregate_id, sequence_number, event_type, payload, timestamp)
             VALUES ('agg-1', 2, 'PrintJobQueued', '{}', ?1)",
            rusqlite::params![now],
        )
        .unwrap();
    }

    #[test]
    fn test_print_jobs_indexes_exist() {
        let mut conn = open_test_conn();
        run_migrations(&mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND tbl_name='print_jobs' AND name LIKE 'idx_%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_events_indexes_exist() {
        let mut conn = open_test_conn();
        run_migrations(&mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND tbl_name='events' AND name LIKE 'idx_%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            count >= 2,
            "Expected at least 2 named indexes on events, got {count}"
        );
    }

    #[test]
    fn test_print_jobs_has_scheduled_at_column() {
        let mut conn = open_test_conn();
        run_migrations(&mut conn).unwrap();

        // Verify column exists
        let columns: Vec<String> = conn
            .prepare("PRAGMA table_info(print_jobs)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert!(
            columns.contains(&"scheduled_at".to_string()),
            "scheduled_at column should exist"
        );

        // Verify index exists
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_print_jobs_scheduled'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "scheduled_at index should exist");
    }

    #[test]
    fn test_print_jobs_has_settings_json_column() {
        let mut conn = open_test_conn();
        run_migrations(&mut conn).unwrap();

        let columns: Vec<String> = conn
            .prepare("PRAGMA table_info(print_jobs)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert!(
            columns.contains(&"settings_json".to_string()),
            "settings_json column should exist"
        );
    }
}
