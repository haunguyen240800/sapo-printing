use rusqlite::Connection;
use rusqlite_migration::{M, Migrations};

use super::connection::DatabaseError;

const MIGRATION_1: &str = "
CREATE TABLE printer_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    printer_name TEXT NOT NULL,
    device_id TEXT NOT NULL UNIQUE,
    printer_type TEXT NOT NULL DEFAULT 'Local',
    status TEXT NOT NULL DEFAULT 'Offline',
    paper_size TEXT NOT NULL DEFAULT 'A4',
    paper_width INTEGER,
    paper_height INTEGER,
    orientation TEXT NOT NULL DEFAULT 'portrait',
    margin_left INTEGER NOT NULL DEFAULT 0,
    margin_right INTEGER NOT NULL DEFAULT 0,
    margin_top INTEGER NOT NULL DEFAULT 0,
    margin_bottom INTEGER NOT NULL DEFAULT 0,
    color_mode TEXT NOT NULL DEFAULT 'RGB',
    print_as_image INTEGER NOT NULL DEFAULT 0,
    enable_buffer INTEGER NOT NULL DEFAULT 0,
    buffer_size_kb INTEGER,
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX idx_printer_configs_device ON printer_configs(device_id);
CREATE INDEX idx_printer_configs_name ON printer_configs(printer_name);
";

const MIGRATION_2: &str = "
CREATE TABLE app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    value_type TEXT NOT NULL,
    description TEXT,
    updated_at INTEGER NOT NULL
);
INSERT INTO app_settings (key, value, value_type, description, updated_at) VALUES
    ('log_level', 'INFO', 'string', 'Logging level: DEBUG, INFO, WARN, ERROR', strftime('%s', 'now')),
    ('max_concurrent_downloads', '10', 'integer', 'Max parallel downloads', strftime('%s', 'now')),
    ('max_concurrent_renders', '10', 'integer', 'Max parallel renders', strftime('%s', 'now')),
    ('default_batch_size', '50', 'integer', 'Default batch size for bulk print', strftime('%s', 'now')),
    ('temp_file_retention_hours', '24', 'integer', 'Hours to keep temp files', strftime('%s', 'now')),
    ('auto_update_enabled', '1', 'boolean', 'Enable auto-update check', strftime('%s', 'now')),
    ('last_update_check', '0', 'integer', 'Unix timestamp of last update check', strftime('%s', 'now'));
";

const MIGRATION_3: &str = "
CREATE TABLE print_jobs (
    id TEXT PRIMARY KEY CHECK(length(id) = 36),
    printer_name TEXT NOT NULL,
    document_url TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'PENDING',
    retry_count INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    completed_at INTEGER
);
CREATE INDEX idx_print_jobs_status ON print_jobs(status);
CREATE INDEX idx_print_jobs_created_at ON print_jobs(created_at);
";

const MIGRATION_4: &str = "
CREATE TABLE events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    aggregate_id TEXT NOT NULL,
    sequence_number INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    hmac TEXT,
    UNIQUE(aggregate_id, sequence_number)
);
CREATE INDEX idx_events_aggregate ON events(aggregate_id);
CREATE INDEX idx_events_type ON events(event_type);
";

const MIGRATION_5: &str = "
ALTER TABLE print_jobs ADD COLUMN scheduled_at INTEGER;
CREATE INDEX idx_print_jobs_scheduled ON print_jobs(scheduled_at);
UPDATE print_jobs SET scheduled_at = updated_at WHERE scheduled_at IS NULL;
";

const MIGRATION_6: &str = "
ALTER TABLE print_jobs ADD COLUMN error_message TEXT;
";

const MIGRATION_7: &str = "
ALTER TABLE print_jobs ADD COLUMN output_path TEXT;
";

const MIGRATION_8: &str = "
CREATE TABLE api_tokens (
    token_hash TEXT PRIMARY KEY,
    token_salt TEXT NOT NULL,
    origin TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    last_used_at INTEGER,
    expires_at INTEGER NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_tokens_origin ON api_tokens(origin);
CREATE INDEX idx_tokens_expires ON api_tokens(expires_at);
";

pub fn run_migrations(conn: &mut Connection) -> Result<(), DatabaseError> {
    let migrations = Migrations::new(vec![
        M::up(MIGRATION_1),
        M::up(MIGRATION_2),
        M::up(MIGRATION_3),
        M::up(MIGRATION_4),
        M::up(MIGRATION_5),
        M::up(MIGRATION_6),
        M::up(MIGRATION_7),
        M::up(MIGRATION_8),
    ]);
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
    fn test_migrations_create_printer_configs_table() {
        let mut conn = open_test_conn();
        run_migrations(&mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='printer_configs'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_migrations_create_app_settings_table() {
        let mut conn = open_test_conn();
        run_migrations(&mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='app_settings'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_migrations_seed_app_settings() {
        let mut conn = open_test_conn();
        run_migrations(&mut conn).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM app_settings", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 7);
    }

    #[test]
    fn test_migrations_idempotent() {
        let mut conn = open_test_conn();
        run_migrations(&mut conn).unwrap();
        run_migrations(&mut conn).unwrap();
    }

    #[test]
    fn test_migrations_create_print_jobs_table() {
        let mut conn = open_test_conn();
        run_migrations(&mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='print_jobs'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_migrations_create_events_table() {
        let mut conn = open_test_conn();
        run_migrations(&mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='events'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_print_jobs_schema_constraints() {
        let mut conn = open_test_conn();
        run_migrations(&mut conn).unwrap();
        let now = 1_700_000_000i64;

        conn.execute(
            "INSERT INTO print_jobs (id, printer_name, document_url, status, created_at, updated_at)
             VALUES ('job-1', 'HP LaserJet', 'https://s3.example.com/doc.pdf', 'PENDING', ?1, ?1)",
            rusqlite::params![now],
        )
        .unwrap();

        let (retry_count, completed_at): (i64, Option<i64>) = conn
            .query_row(
                "SELECT retry_count, completed_at FROM print_jobs WHERE id = 'job-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(retry_count, 0);
        assert!(completed_at.is_none());

        let result = conn.execute(
            "INSERT INTO print_jobs (id, printer_name, document_url, status, created_at, updated_at)
             VALUES ('job-1', 'Another Printer', 'https://s3.example.com/doc2.pdf', 'PENDING', ?1, ?1)",
            rusqlite::params![now],
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
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND tbl_name='print_jobs'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 2);
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
    fn test_migration_5_adds_scheduled_at_column() {
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
}
