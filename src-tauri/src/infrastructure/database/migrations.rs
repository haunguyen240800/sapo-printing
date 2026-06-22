use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};

use super::connection::DatabaseError;

const MIGRATION_1: &str = "
CREATE TABLE printer_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    printer_name TEXT NOT NULL,
    device_id TEXT NOT NULL UNIQUE,
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

pub fn run_migrations(conn: &mut Connection) -> Result<(), DatabaseError> {
    let migrations = Migrations::new(vec![M::up(MIGRATION_1), M::up(MIGRATION_2)]);
    migrations
        .to_latest(conn)
        .map_err(|e| DatabaseError::MigrationFailed { reason: e.to_string() })
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
}
