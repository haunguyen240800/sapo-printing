# Investigation: DatabaseTooFarAhead after baseline migration squash

## Hand-off Brief

1. **What happened.** Confirmed: app startup fails because the local SQLite schema version is higher than the new single-migration baseline.
2. **Where the case stands.** Root cause is localized; neither the active DB nor its WAL/SHM files has been modified.
3. **What's needed next.** Choose a one-time development DB reset or authorize an automatic reset policy.

## Case Info

| Field | Value |
| --- | --- |
| Ticket | N/A |
| Date opened | 2026-08-24 |
| Status | Active |
| System | Windows, Tauri dev build, sapo-printer-pro-max 1.0.9 |
| Evidence sources | User startup log, source code, local filesystem, commit `3c8944f` |

## Problem Statement

User ran `pnpm dev`; startup exited with `MigrationDefinition(DatabaseTooFarAhead)` immediately after the migration baseline was squashed from nine entries to one.

## Evidence Inventory

| Source | Status | Notes |
| --- | --- | --- |
| User startup log, 2026-08-24 16:27 | Available | Direct error: `MigrationDefinition(DatabaseTooFarAhead)` |
| `src-tauri/src/infrastructure/configs/db/migrations.rs` | Available | Runtime now registers exactly one migration |
| `src-tauri/src/bootstrap/database.rs` | Available | Migration failure exits process with code 1 |
| Active data directory | Available | Roaming DB plus `config.db-wal` and `config.db-shm` exist |
| SQLite data inspection | Partial | Exact legacy `user_version` not queried; error proves it is ahead of baseline version 1 |

## Investigation Backlog

| # | Path to Explore | Priority | Status | Notes |
| - | --- | --- | --- | --- |
| 1 | Choose recovery policy | High | Open | Manual development reset or guarded automatic reset |
| 2 | Verify clean startup after chosen recovery | High | Blocked | Requires user authorization for destructive reset or implementation |

## Timeline of Events

| Time | Event | Source | Confidence |
| --- | --- | --- | --- |
| Before 2026-08-24 | Local DB received historical migrations up to a version greater than 1 | Migration error and prior source | Deduced |
| 2026-08-24 | Commit `3c8944f` squashed migration definitions into one baseline | Git commit and `migrations.rs:58` | Confirmed |
| 2026-08-24 16:27 | Dev app opened existing DB and exited during migration | User log | Confirmed |

## Confirmed Findings

### Finding 1: Runtime baseline is behind the local DB

**Evidence:** `src-tauri/src/infrastructure/configs/db/migrations.rs:58`; user log at 2026-08-24 16:27.

**Detail:** `Migrations::new` now contains one entry. `rusqlite_migration` rejects the existing higher schema version as `DatabaseTooFarAhead`.

### Finding 2: Migration failure is fatal during bootstrap

**Evidence:** `src-tauri/src/bootstrap/database.rs:22`.

**Detail:** `run_migrations` failure prints the error and calls `std::process::exit(1)`.

### Finding 3: The active DB is in the Roaming app data directory

**Evidence:** `src-tauri/src/bootstrap/dirs.rs:35`; filesystem observation.

**Detail:** Active files are `C:\Users\HauNV-PC\AppData\Roaming\sapo-printer-pro-max\config.db`, `config.db-wal`, and `config.db-shm`. A separate `C:\Users\HauNV-PC\.sapo-printer\config.db` exists but is not selected by current bootstrap code.

## Deduced Conclusions

### Deduction 1: A fresh DB will start successfully

**Based on:** Finding 1 and the passing in-memory migration tests.

**Reasoning:** The error is version comparison before applying SQL, while the same baseline succeeds against version 0.

**Conclusion:** Removing or relocating the complete active SQLite file set will let bootstrap create the new schema.

## Hypothesized Paths

### Hypothesis 1: Local database is on historical migration version 9

**Status:** Open

**Theory:** The DB was last run with the former nine-migration list.

**Supporting indicators:** Previous source registered nine migrations and the current error requires version greater than 1.

**Would confirm:** Read `PRAGMA user_version` from the active DB after safely closing all app processes.

**Would refute:** A returned version between 2 and 8.

**Resolution:** Not required to establish root cause; exact version only affects forensic detail.

## Missing Evidence

| Gap | Impact | How to Obtain |
| --- | --- | --- |
| User's preferred reset policy | Determines whether recovery is manual or implemented | User choice |
| Exact `PRAGMA user_version` | Distinguishes historical version 2–9 | Query DB while app is stopped |

## Source Code Trace

| Element | Detail |
| --- | --- |
| Error origin | `src-tauri/src/infrastructure/configs/db/migrations.rs:58`, `run_migrations` |
| Trigger | Tauri setup calls `bootstrap::database::init_database` |
| Condition | Existing SQLite `user_version` is greater than migration list length 1 |
| Related files | `src-tauri/src/bootstrap/database.rs:11`, `src-tauri/src/bootstrap/dirs.rs:35` |

## Conclusion

**Confidence:** High

The baseline squash is correct for a fresh unreleased installation but incompatible with the existing development DB's migration version. This is deterministic and not caused by PDFium, Tauri, Vite, or the Chrome window error printed during process teardown.

## Recommended Next Steps

### Fix direction

Recommended: stop the app, move the complete active SQLite set (`config.db`, `config.db-wal`, `config.db-shm`) to a backup folder, then rerun `pnpm dev`. Automatic deletion inside production bootstrap is not recommended unless explicitly authorized.

### Diagnostic

Optionally read `PRAGMA user_version` before reset for audit evidence.

## Reproduction Plan

1. Start with a DB migrated by the former nine-entry migration list.
2. Run app code containing the new one-entry baseline.
3. Observe `DatabaseTooFarAhead` and exit code 1.
4. Repeat with an empty DB path; expect baseline migration and startup to succeed.

## Side Findings

- Confirmed: the final Chrome class-unregister error occurs after the fatal migration exit and is secondary.

## Follow-up: 2026-08-24

### New Evidence

- `src-tauri/src/infrastructure/configs/app/app_print_config.rs:50` hard-codes `.sapo-printer` below `USERPROFILE`/`HOME`.
- `src-tauri/src/infrastructure/integrations/network/reqwest_downloader.rs:198` independently hard-codes `.sapo-printer/temp`.
- `src-tauri/src/bootstrap/dirs.rs:5` uses generated `SAPO_APP_SLUG`; `build.rs:15-17` derives it from `tauri.conf.json.productName`, producing `sapo-printer-pro-max`.

### Additional Findings

**Confirmed:** folder naming is inconsistent because print config and downloader bypass the central `AppDirs` resolver. Renaming `productName` only affects paths using `SAPO_APP_SLUG`; it cannot alter string literals in the two adapters.

**Confirmed:** this is also a pipeline defect, not only cosmetic. The downloader returns a file under `~/.sapo-printer/temp`, while `FilesystemTempFileManager` is initialized with the OS data directory's `sapo-printer-pro-max/temp`; `TempPdfFile::try_new` rejects files outside that directory.

### Updated Hypotheses

- Hypothesis: all runtime data already follows `sapo-printer-pro-max`. **Refuted** by the two hard-coded paths above.

### Backlog Changes

- Add: centralize print config and downloader temp paths on `AppDirs`, then update `CLAUDE.md` and tests.

### Updated Conclusion

The old folder persists because two infrastructure adapters still explicitly construct `.sapo-printer`. The correct fix is dependency injection of resolved app/temp paths, not another literal rename.
