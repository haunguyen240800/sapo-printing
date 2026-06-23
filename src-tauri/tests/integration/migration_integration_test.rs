use sapo_printer::infrastructure::database::{run_migrations, DbPool};

struct TempDir(std::path::PathBuf);
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn test_fresh_database_runs_all_migrations_and_can_insert_query() {
    let dir = TempDir(std::env::temp_dir().join(format!("sapo_inttest_{}", uuid::Uuid::new_v4())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let db_path = dir.0.join("test.db");

    let pool = DbPool::new(db_path.to_str().unwrap()).unwrap();
    {
        let mut conn = pool.get();
        run_migrations(&mut conn).unwrap();
    }

    // Insert print job
    {
        let conn = pool.get();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        conn.execute(
            "INSERT INTO print_jobs (id, printer_name, document_url, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "test-job-id",
                "HP LaserJet",
                "https://s3.example.com/doc.pdf",
                "PENDING",
                now,
                now
            ],
        )
        .unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM print_jobs WHERE id = 'test-job-id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    // Insert event
    {
        let conn = pool.get();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        conn.execute(
            "INSERT INTO events (aggregate_id, sequence_number, event_type, payload, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                "test-job-id",
                1,
                "PrintJobCreated",
                r#"{"job_id":"test-job-id"}"#,
                now
            ],
        )
        .unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM events WHERE aggregate_id = 'test-job-id'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    drop(pool);
    // dir dropped here — TempDir::drop handles cleanup even on panic
}
