---
baseline_commit: a48f7645d9c0ba4226025ac9fcbf1345131aee58
---

# Story 2.1: Setup SQLite Database with Migrations & Schemas

Status: done

## Story

As a **developer**,
I want **to setup SQLite database with migration system and create printer-related schemas**,
So that **the application can persist printer configurations and the database can evolve safely across versions**.

## Acceptance Criteria

**Given** the infrastructure layer structure exists (Epic 1 done)
**When** I implement the database setup

**AC-1: Database Connection**
- `src-tauri/src/infrastructure/database/connection.rs` must be created with:
  - `DbPool` struct wrapping `Arc<Mutex<Connection>>`
  - `DbPool::new(db_path: &str) -> Result<Self, DatabaseError>` — opens SQLite connection, enables WAL mode and `PRAGMA synchronous=NORMAL`
  - `DbPool::get(&self) -> MutexGuard<Connection>` — returns lock guard for use
  - `DatabaseError` enum with at least `ConnectionFailed { reason: String }` and `MigrationFailed { reason: String }`

**AC-2: Migrations — Migration 1: printer_configs + indexes**
- `src-tauri/src/infrastructure/database/migrations.rs` must use `rusqlite_migration::Migrations`
- Migration 1 must create `printer_configs` table:
  ```sql
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
  ```

**AC-3: Migrations — Migration 2: app_settings + seed data**
- Migration 2 must create `app_settings` table and insert 7 default rows:
  ```sql
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
  ```

**AC-4: Migration runner**
- `migrations.rs` must expose `pub fn run_migrations(conn: &mut Connection) -> Result<(), DatabaseError>`
- Migrations must be applied idempotently (safe to run on already-migrated DB)
- `DbPool::new()` must NOT call migrations automatically — caller (`main.rs`) drives initialization

**AC-5: Unit Conversion Utilities**
- `src-tauri/src/shared/utils/unit_conversion.rs` must be created with:
  - `pub fn mm_to_pixels(mm: f64, dpi: u32) -> u32` — `(mm / 25.4 * dpi as f64).round() as u32`
  - `pub fn pixels_to_mm(pixels: u32, dpi: u32) -> f64` — `pixels as f64 / dpi as f64 * 25.4`
  - `pub fn mm_to_inches(mm: f64) -> f64` — `mm / 25.4`
  - `pub fn inches_to_mm(inches: f64) -> f64` — `inches * 25.4`
- `src-tauri/src/shared/utils/mod.rs` must declare `pub mod unit_conversion;`

**AC-6: main.rs initialization stub**
- `src-tauri/src/main.rs` must be updated to:
  - Create `DbPool` pointing to `~/.sapo-printer/config.db` (use `dirs` crate or hardcode path with `std::env::home_dir()`)
  - Call `run_migrations(&mut conn)` before `tauri::Builder`
  - Log any init error to stderr and `std::process::exit(1)` on failure

**AC-7: Module wiring**
- `src-tauri/src/infrastructure/database/mod.rs` must declare:
  - `pub mod connection;`
  - `pub mod migrations;`
  - Re-export: `pub use connection::{DbPool, DatabaseError};`
  - Re-export: `pub use migrations::run_migrations;`
- `src-tauri/src/infrastructure/mod.rs` must expose `pub mod database;`

**AC-8: Unit Tests**
- `connection.rs` inline tests:
  - `test_db_pool_opens_wal_mode` — open `:memory:`, verify `PRAGMA journal_mode` returns `"wal"`
- `migrations.rs` inline tests:
  - `test_migrations_create_printer_configs_table` — run migrations on `:memory:`, query `sqlite_master` to confirm table exists
  - `test_migrations_create_app_settings_table` — same check for `app_settings`
  - `test_migrations_seed_app_settings` — verify 7 rows inserted
  - `test_migrations_idempotent` — run `run_migrations` twice, expect no error
- `unit_conversion.rs` inline tests:
  - `test_mm_to_pixels_a4_width` — `mm_to_pixels(210.0, 300)` == `2480`
  - `test_mm_to_pixels_a4_height` — `mm_to_pixels(297.0, 300)` == `3508`
  - `test_pixels_to_mm_roundtrip` — `pixels_to_mm(mm_to_pixels(210.0, 300), 300)` ≈ 210.0 (±0.5mm tolerance)
  - `test_mm_to_inches` — `mm_to_inches(25.4)` == `1.0`
  - `test_inches_to_mm` — `inches_to_mm(1.0)` == `25.4`

**AC-9: All Tests Pass**
- `cargo test` — all tests pass, no regressions from Epic 1 (91+ tests)
- `cargo build` — zero errors
- `cargo clippy` — zero warnings

## Tasks / Subtasks

- [x] **Task 1: DatabaseError type** (AC: #1)
  - [x] Create `src-tauri/src/infrastructure/database/connection.rs`
  - [x] Define `DatabaseError` enum: `ConnectionFailed { reason: String }`, `MigrationFailed { reason: String }`
  - [x] Implement `Display` + `std::error::Error` for `DatabaseError`

- [x] **Task 2: DbPool connection** (AC: #1)
  - [x] Implement `DbPool` struct wrapping `Arc<Mutex<Connection>>`
  - [x] `DbPool::new(db_path: &str) -> Result<Self, DatabaseError>` — open connection, set WAL mode
  - [x] `DbPool::get(&self) -> MutexGuard<Connection>`
  - [x] Add inline test: `test_db_pool_opens_wal_mode`

- [x] **Task 3: Migrations — Migration 1** (AC: #2)
  - [x] Create `src-tauri/src/infrastructure/database/migrations.rs`
  - [x] Define `printer_configs` CREATE TABLE SQL (exact schema from AC-2)
  - [x] Add indexes on `device_id` and `printer_name`

- [x] **Task 4: Migrations — Migration 2** (AC: #3)
  - [x] Define `app_settings` CREATE TABLE SQL
  - [x] INSERT 7 default settings rows using `strftime('%s', 'now')`

- [x] **Task 5: run_migrations function** (AC: #4)
  - [x] `pub fn run_migrations(conn: &mut Connection) -> Result<(), DatabaseError>`
  - [x] Use `rusqlite_migration::Migrations::new(vec![M::up(...), M::up(...)])` with both migrations
  - [x] Wrap errors into `DatabaseError::MigrationFailed`
  - [x] Add inline tests: create + seed + idempotency

- [x] **Task 6: Unit conversion utilities** (AC: #5)
  - [x] Create `src-tauri/src/shared/utils/unit_conversion.rs`
  - [x] Implement 4 functions: `mm_to_pixels`, `pixels_to_mm`, `mm_to_inches`, `inches_to_mm`
  - [x] Update `shared/utils/mod.rs` to declare `pub mod unit_conversion;`
  - [x] Add 5 inline tests (A4 dimensions, roundtrip, inches)

- [x] **Task 7: Wire modules** (AC: #7)
  - [x] Update `infrastructure/database/mod.rs` — add submodules + re-exports
  - [x] Verify `infrastructure/mod.rs` exposes `pub mod database;` (add if missing)

- [x] **Task 8: main.rs initialization** (AC: #6)
  - [x] Update `src-tauri/src/main.rs`:
    - Create `~/.sapo-printer/` directory if not exists
    - Open `DbPool` for `config.db`
    - Call `run_migrations`
    - Exit on error

- [x] **Task 9: Verify** (AC: #9)
  - [x] `cargo build` — zero errors
  - [x] `cargo test` — all pass (101 tests: 91 existing + 10 new)
  - [x] `cargo clippy` — zero warnings

## Dev Notes

### 🎯 Story Purpose

Story 2.1 là **foundation của Epic 2** — không có DB schema và connection, không story nào trong Epic 2 có thể implement. Story này chỉ tạo infrastructure, không có domain logic mới.

**Scope rõ ràng:**
- NEW: `infrastructure/database/connection.rs`, `infrastructure/database/migrations.rs`
- NEW: `shared/utils/unit_conversion.rs`
- MODIFIED: `infrastructure/database/mod.rs`, `infrastructure/mod.rs`, `shared/utils/mod.rs`, `main.rs`
- DO NOT touch: domain layer, secrets, queue, renderer, printer infrastructure

### 🏗️ Exact File Changes

```
NEW files:
  src-tauri/src/infrastructure/database/connection.rs
  src-tauri/src/infrastructure/database/migrations.rs
  src-tauri/src/shared/utils/unit_conversion.rs

MODIFIED files:
  src-tauri/src/infrastructure/database/mod.rs   (replace stub comment with modules)
  src-tauri/src/infrastructure/mod.rs             (verify pub mod database exists)
  src-tauri/src/shared/utils/mod.rs               (add pub mod unit_conversion)
  src-tauri/src/main.rs                           (add DB initialization before Builder)
```

### 📦 Code Specifications

```rust
// infrastructure/database/connection.rs
use rusqlite::{Connection, MutexGuard};  // NOTE: MutexGuard is std, not rusqlite
use std::sync::{Arc, Mutex};
use std::fmt;

#[derive(Debug)]
pub enum DatabaseError {
    ConnectionFailed { reason: String },
    MigrationFailed { reason: String },
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DatabaseError::ConnectionFailed { reason } => write!(f, "DB connection failed: {}", reason),
            DatabaseError::MigrationFailed { reason } => write!(f, "DB migration failed: {}", reason),
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

    pub fn get(&self) -> std::sync::MutexGuard<Connection> {
        self.0.lock().expect("DB mutex poisoned")
    }
}
```

```rust
// infrastructure/database/migrations.rs
use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};
use super::connection::DatabaseError;

pub fn run_migrations(conn: &mut Connection) -> Result<(), DatabaseError> {
    let migrations = Migrations::new(vec![
        M::up(MIGRATION_1),
        M::up(MIGRATION_2),
    ]);
    migrations.to_latest(conn)
        .map_err(|e| DatabaseError::MigrationFailed { reason: e.to_string() })
}

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
```

```rust
// shared/utils/unit_conversion.rs
/// Convert millimeters to pixels at given DPI
pub fn mm_to_pixels(mm: f64, dpi: u32) -> u32 {
    (mm / 25.4 * dpi as f64).round() as u32
}

/// Convert pixels to millimeters at given DPI
pub fn pixels_to_mm(pixels: u32, dpi: u32) -> f64 {
    pixels as f64 / dpi as f64 * 25.4
}

/// Convert millimeters to inches
pub fn mm_to_inches(mm: f64) -> f64 {
    mm / 25.4
}

/// Convert inches to millimeters
pub fn inches_to_mm(inches: f64) -> f64 {
    inches * 25.4
}
```

```rust
// main.rs update pattern
fn main() {
    // 1. Ensure data directory exists
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let data_dir = std::path::PathBuf::from(&home).join(".sapo-printer");
    std::fs::create_dir_all(&data_dir).expect("Cannot create data directory");
    let db_path = data_dir.join("config.db");

    // 2. Initialize database
    let pool = sapo_printer::infrastructure::database::DbPool::new(
        db_path.to_str().unwrap()
    ).unwrap_or_else(|e| {
        eprintln!("Database init failed: {e}");
        std::process::exit(1);
    });
    {
        let mut conn = pool.get();
        sapo_printer::infrastructure::database::run_migrations(&mut conn)
            .unwrap_or_else(|e| {
                eprintln!("Migration failed: {e}");
                std::process::exit(1);
            });
    }

    // 3. Start Tauri
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("Tauri error: {e}");
            std::process::exit(1);
        });
}
```

**Note về main.rs:** `main.rs` là binary entry point, gọi functions từ `lib.rs`. Nếu project structure dùng `lib.rs` (đã có), cần expose modules qua `lib.rs`. Kiểm tra `lib.rs` hiện tại và adjust import path cho phù hợp.

### ⚠️ Dependency Check

Tất cả dependencies đã có trong `Cargo.toml`:
- `rusqlite = { version = "0.32", features = ["bundled"] }` ✅
- `rusqlite_migration = "1.2"` ✅
- **KHÔNG thêm dependency mới**

### ⚠️ main.rs vs lib.rs Pattern

Kiểm tra `src-tauri/src/lib.rs` trước khi sửa `main.rs`:
- Nếu `lib.rs` expose infrastructure modules → import từ `sapo_printer::infrastructure::database`
- Nếu không → gọi trực tiếp từ `main.rs` với `use crate::infrastructure::database`

Hiện tại `main.rs` chỉ có:
```rust
fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        ...
}
```
Cần add DB init block TRƯỚC `.run()`.

### 🔧 Migration SQL — Critical Details

1. **Integers cho booleans**: SQLite không có `BOOLEAN` type — dùng `INTEGER NOT NULL DEFAULT 0` ✅
2. **`strftime('%s', 'now')`**: returns Unix timestamp as TEXT trong SQLite — OK cho `INTEGER` column type khi insert vào `app_settings.updated_at`
3. **Migration idempotency**: `rusqlite_migration` tracks applied migrations trong `rusqlite_migrations` internal table. Re-running `to_latest()` trên DB đã migrate là NO-OP ✅

### 🧪 Test Pattern Examples

```rust
// connection.rs tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_pool_opens_wal_mode() {
        let pool = DbPool::new(":memory:").unwrap();
        let conn = pool.get();
        let mode: String = conn
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }
}

// migrations.rs tests
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
        run_migrations(&mut conn).unwrap(); // second run must not fail
    }
}
```

### 🚫 Anti-Patterns (DO NOT)

1. ❌ DO NOT touch `domain/` layer — zero changes
2. ❌ DO NOT implement PrinterRepository here — that's Story 2.4
3. ❌ DO NOT add `print_jobs` or `events` tables — those belong to Story 3.6
4. ❌ DO NOT use `async` for DB operations — synchronous Mutex pattern per architecture
5. ❌ DO NOT add new Cargo.toml dependencies — all present
6. ❌ DO NOT call `DbPool::new()` in tests without `:memory:` — use in-memory only
7. ❌ DO NOT expose `DbPool.0` field directly — only via `.get()`

### 📚 Architecture References

- AR-4 (Repository Pattern): `_bmad-output/planning-artifacts/architecture.md` — Decision 11 (SQLite connection)
- AR-8 (Database Schema): `_bmad-output/planning-artifacts/architecture.md` — Decision 1.2 (Complete schemas)
- AR-1 (Clean Architecture): Infrastructure layer owns all DB concerns
- `unit_conversion.rs` location: `shared/utils/` — architecture doc § "Unit Conversion Utilities"
- WAL mode: Decision 11 in architecture — `PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;`

### 🔗 Story Dependencies

- **Depends on**: Epic 1 all done ✅ (1.1 → 1.4)
- **Enables**: 
  - Story 2.2 (Windows printer discovery) — needs DB for status caching
  - Story 2.4 (PrinterRepository) — needs `printer_configs` table ← primary consumer
  - Story 3.6 (Print job tables) — will add Migration 3 + 4 on top of this

### 📊 Epic 2 Context

Story 2.1 là story đầu tiên của Epic 2. Sau khi tạo story này:
- Epic 2 status sẽ chuyển từ `backlog` → `in-progress` (tự động khi tạo story)
- Dev sẽ implement connection + migrations + unit_conversion
- Stories 2.2, 2.3, 2.4 phụ thuộc vào schema `printer_configs` được tạo ở đây

## Dev Agent Record

### Agent Model Used

Claude Sonnet 4.5 (via Kiro)

### Completion Notes List

- AC-1: `DbPool` wraps `Arc<Mutex<Connection>>` với WAL mode. Test dùng file tạm vì SQLite không hỗ trợ WAL cho `:memory:`.
- AC-2/3: Hai migrations định nghĩa qua `rusqlite_migration` — `printer_configs` (17 columns + 2 indexes) và `app_settings` (7 seed rows).
- AC-4: `run_migrations` idempotent — `rusqlite_migration` tự track applied migrations.
- AC-5: 4 unit conversion functions với 5 tests — A4 dimensions verified (2480px, 3508px @ 300dpi).
- AC-6: `main.rs` khởi tạo `~/.sapo-printer/config.db`, chạy migrations, exit(1) nếu lỗi.
- AC-7: `database/mod.rs` re-export `DbPool`, `DatabaseError`, `run_migrations`.
- AC-8/9: 101 tests pass (10 mới), zero build errors, zero clippy warnings.

### File List

**New files:**
- `src-tauri/src/infrastructure/database/connection.rs`
- `src-tauri/src/infrastructure/database/migrations.rs`
- `src-tauri/src/shared/utils/unit_conversion.rs`

**Modified files:**
- `src-tauri/src/infrastructure/database/mod.rs`
- `src-tauri/src/infrastructure/mod.rs`
- `src-tauri/src/shared/utils/mod.rs`
- `src-tauri/src/main.rs`

## Change Log

**2026-06-22: Story created by Amelia (bmad-create-story)**
**2026-06-22: Story implemented by Amelia (bmad-dev-story)** — Added `connection.rs`, `migrations.rs`, `unit_conversion.rs`; wired modules; updated `main.rs` with DB init. 101 tests pass, zero warnings.


### Review Findings

**Code review by Amelia — 2026-06-22** | Blind Hunter + Edge Case Hunter + Acceptance Auditor

Patches applied (6), Deferred (6), Dismissed (7):

- [x] [Review][Patch] `pool` not registered in Tauri state — added `.manage(pool)` to `tauri::Builder` [main.rs]
- [x] [Review][Patch] `db_path.to_str().unwrap()` panic on non-UTF-8 path — replaced with `unwrap_or_else` + exit [main.rs]
- [x] [Review][Patch] `dpi=0` in `pixels_to_mm` causes division-by-zero — added `assert!(dpi > 0)` guard [unit_conversion.rs]
- [x] [Review][Patch] Test temp filename collision risk (`subsec_nanos` only) — added thread ID to filename [connection.rs]
- [x] [Review][Patch] Temp DB file not dropped before `remove_file` on Windows — added explicit `drop(pool)` [connection.rs]
- [x] [Review][Patch] D3 verified: `infrastructure/mod.rs` already has `pub mod database` — no change needed
- [x] [Review][Defer] Home dir fallback `"."` if env vars unset — cross-platform concern for Epic 4 — deferred, pre-existing
- [x] [Review][Defer] No DOWN migrations / rollback path — Epic 4 concern — deferred, pre-existing
- [x] [Review][Defer] `color_mode`/`orientation` lack CHECK constraints — Story 2.4 schema validation concern — deferred, pre-existing
- [x] [Review][Defer] Boolean columns lack `CHECK(value IN (0,1))` — Story 2.4 concern — deferred, pre-existing
- [x] [Review][Defer] `get()` panics on mutex poison instead of Result — design tradeoff, acceptable for now — deferred, pre-existing
- [x] [Review][Defer] Negative/NaN `mm` input to unit conversion — no domain validation layer yet — deferred, pre-existing
