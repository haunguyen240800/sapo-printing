---
title: "Story 3.5: Implement RAII Temp File Management with Tiered Cleanup"
status: review
story_id: "3.5"
story_key: "3-5-implement-raii-temp-file-management-with-tiered-cleanup"
epic: 3
parent_story: 3-4-create-hybrid-strategy-selector-auto-detect-printer-capability
baseline_commit: a7850cd
created: 2026-06-23
---

# Story 3.5: Implement RAII Temp File Management with Tiered Cleanup

Status: done

## Story

As a **developer**,
I want **automatic temp file cleanup with RAII pattern and tiered retention**,
So that **disk space is reclaimed immediately on success, but failed jobs are kept for 24h debugging, and orphaned files are removed on startup**.

## Context

Story này là **story thứ năm trong Epic 3** — implement AR-7 (RAII Pattern for Resource Cleanup). Story này hoàn toàn độc lập: không cần PrintJobRepository hay EventBus, chỉ cần hiểu temp file lifecycle.

**Business Value:**
- NFR-1: `Disk cleanup temp files ngay sau job complete` — disk space không bị lãng phí
- NFR-2: Failed jobs giữ lại 24h để debug rồi tự xóa
- Startup cleanup đảm bảo không có orphaned files từ crash

**Architecture Context (AR-7):**
- `TempPdfFile` struct với `Drop` trait — auto-deletes khi out of scope nếu `keep_on_drop = false`
- Tiered cleanup strategy:
  1. **Immediate** (Success): Drop với `keep_on_drop = false` → xóa ngay
  2. **Deferred** (Failure): Drop với `keep_on_drop = true` → giữ 24h
  3. **Startup**: Scan `~/.sapo-printer/temp/` → xóa `.tmp` + `.pdf` cũ hơn 24h
- `startup_cleanup()` function được gọi trong `main.rs`

**Current Codebase State:**
- `ReqwestDownloader` tạo files tại `~/.sapo-printer/temp/{job_id}.pdf` — hiện KHÔNG dùng `TempPdfFile`
- Không có `infrastructure/temp_file.rs` nào tồn tại — file này là NEW
- `main.rs` có data dir setup tại `~/.sapo-printer/` nhưng chưa có `temp/` subdirectory creation
- `InfrastructureError` đã có `ValidationError`, `NetworkError` — dùng lại cho cleanup errors
- `home` crate đã có trong dependencies (dùng trong `reqwest_downloader.rs`)

**Blocks:** Story 3.5 (Queue Worker) sẽ dùng `TempPdfFile` để wrap path trả về từ downloader.

## Acceptance Criteria

### AC-1: TempPdfFile Struct với RAII Drop

**Given** the downloader creates PDF files at `~/.sapo-printer/temp/{job_id}.pdf`
**When** I implement TempPdfFile
**Then** `src-tauri/src/infrastructure/temp_file.rs` must be created:

```rust
/// RAII wrapper for a temporary PDF file.
///
/// Automatically deletes the file when dropped unless `keep_on_drop` is true.
///
/// # Cleanup Tiers
/// - Immediate (success): keep_on_drop = false → deleted on Drop
/// - Deferred (failure): keep_on_drop = true → file survives Drop (startup cleanup handles it)
pub struct TempPdfFile {
    path: PathBuf,
    keep_on_drop: bool,
}
```

**Required methods:**
- `pub fn new(path: PathBuf) -> Self` — creates with `keep_on_drop = false` (default: immediate cleanup)
- `pub fn path(&self) -> &Path` — returns reference to the path
- `pub fn keep(&mut self)` — sets `keep_on_drop = true` (call when job FAILED)
- `pub fn release(mut self) -> PathBuf` — sets `keep_on_drop = true` AND returns path without deleting (useful for testing or passing path to next component)

**Drop implementation:**
```rust
impl Drop for TempPdfFile {
    fn drop(&mut self) {
        if !self.keep_on_drop {
            if let Err(e) = std::fs::remove_file(&self.path) {
                // Silently ignore NotFound — file may have been cleaned up by startup_cleanup
                // racing with this Drop (e.g., app restarted while file was in use)
                if e.kind() != std::io::ErrorKind::NotFound {
                    // Log warning but do NOT panic — Drop must not panic
                    tracing::warn!("Failed to cleanup temp file {:?}: {}", self.path, e);
                }
            }
        }
    }
}
```

**CRITICAL:** Drop MUST NOT panic. `ErrorKind::NotFound` must be silently ignored (not warned). All other errors → `tracing::warn!` only.

### AC-2: Startup Cleanup Function

**Given** the app starts and there may be orphaned temp files from previous crashes
**When** I implement startup cleanup
**Then** a `pub fn startup_cleanup(temp_dir: &Path)` function must be created in `src-tauri/src/infrastructure/temp_file.rs`:

**Cleanup logic:**
1. If `temp_dir` does not exist → create it and return (no error)
2. Read all entries in `temp_dir`
3. For each file:
   - If extension is `.tmp` → delete immediately (incomplete downloads from crash)
   - If extension is `.pdf` AND file's modified time > 24h ago → delete (deferred cleanup)
   - Otherwise → skip (recent files kept for failed job debugging)
4. Log summary: `tracing::info!("Startup cleanup: {} files removed, {} kept", removed, kept)`

**Error handling:**
- `read_dir` failure → `tracing::warn!` and return (don't crash app)
- Individual file deletion failure → `tracing::warn!` and continue (best-effort)
- `metadata()` failure (to get modified time) → treat file as old, delete it (safe default)
- **Directory entries** (subdirectories inside `temp_dir`) → skip entirely (check `entry.file_type().map(|t| t.is_file()).unwrap_or(false)` before processing)

**Signature:**
```rust
pub fn startup_cleanup(temp_dir: &Path) {
    // implementation
}
```

### AC-3: Module Registration

**Given** `temp_file.rs` is created in the infrastructure layer
**When** I update module exports
**Then** `src-tauri/src/infrastructure/mod.rs` must be updated:
```rust
pub mod temp_file;
```

**Re-export in `src-tauri/src/lib.rs`** (already exists — verify `infrastructure` module is public):
- No change needed if `pub mod infrastructure;` already present in `lib.rs`

### AC-4: main.rs Integration — Temp Dir Setup + Startup Cleanup

**Given** `startup_cleanup()` exists and `main.rs` already creates `~/.sapo-printer/`
**When** I update main.rs
**Then** add after the data dir creation block (around line 197):

```rust
// Create temp directory for PDF downloads
let temp_dir = data_dir.join("temp");
std::fs::create_dir_all(&temp_dir).unwrap_or_else(|e| {
    eprintln!("Cannot create temp directory: {e}");
    std::process::exit(1);
});

// Startup cleanup: remove orphaned .tmp files and old .pdf files (>24h)
sapo_printer::infrastructure::temp_file::startup_cleanup(&temp_dir);
```

**IMPORTANT:** Insert this BEFORE `DbPool::new()` call (line 204) to ensure cleanup happens early.

### AC-5: Inline Unit Tests

**Given** the implementation is complete
**When** I write inline unit tests in `temp_file.rs`
**Then** `#[cfg(test)] mod tests` must verify:

1. **`test_temp_file_deleted_on_drop_when_keep_false`:**
   - Create a real temp file on disk
   - Create `TempPdfFile::new(path)` with `keep_on_drop = false`
   - Drop the TempPdfFile
   - Assert file no longer exists on disk

2. **`test_temp_file_kept_on_drop_when_keep_true`:**
   - Create a real temp file on disk
   - Create `TempPdfFile::new(path)`, call `.keep()`
   - Drop the TempPdfFile
   - Assert file still exists on disk
   - Manual cleanup after test

3. **`test_temp_file_path_accessible`:**
   - Create TempPdfFile with a path
   - Assert `temp_file.path() == &expected_path`
   - Release to prevent deletion

4. **`test_startup_cleanup_removes_tmp_files`:**
   - Create temp dir with `.tmp` file
   - Run `startup_cleanup(&temp_dir)`
   - Assert `.tmp` file deleted

5. **`test_startup_cleanup_removes_old_pdf`:**
   - Create temp dir with `.pdf` file, set modified time to 25h ago
   - Run `startup_cleanup(&temp_dir)`
   - Assert old `.pdf` deleted

6. **`test_startup_cleanup_keeps_recent_pdf`:**
   - Create temp dir with `.pdf` file (just created, < 24h)
   - Run `startup_cleanup(&temp_dir)`
   - Assert recent `.pdf` still exists
   - Manual cleanup

7. **`test_startup_cleanup_nonexistent_dir_creates_it`:**
   - Path that does not exist
   - Run `startup_cleanup(&nonexistent_dir)`
   - Assert no panic, dir now exists

8. **`test_drop_no_panic_when_file_already_deleted`:**
   - Create TempPdfFile for a path that does NOT exist on disk
   - Drop it — must NOT panic (file not found should be silently swallowed)

9. **`test_temp_pdf_file_is_send`** (E-1 — thread safety for Queue Worker):
   ```rust
   fn assert_send<T: Send>() {}
   #[test]
   fn test_temp_pdf_file_is_send() {
       assert_send::<TempPdfFile>(); // Compile-time check
   }
   ```
   Queue Worker chạy trong background thread — `TempPdfFile` phải là `Send`.

10. **`test_release_does_not_delete`:**
    - Create a real temp file on disk
    - Create `TempPdfFile::new(path)`, call `.release()` → nhận lại `PathBuf`
    - Assert file vẫn còn tồn tại sau khi `TempPdfFile` đã bị consume
    - Manual cleanup

**Test helper:** Use `std::env::temp_dir().join(format!("sapo_test_{}", uuid::Uuid::new_v4()))` for isolated test directories.

**Setting file modified time in tests:**

`filetime` crate **KHÔNG có** trong `Cargo.toml` hiện tại (chỉ có `mockito = "1.0"` trong `[dev-dependencies]`). Cần thêm vào trước khi viết test 5. Xem AC-5 Task 4 và mục Cargo.toml Additions.

```rust
// Sau khi thêm filetime vào [dev-dependencies]:
use std::time::{SystemTime, Duration};
let old_time = SystemTime::UNIX_EPOCH + Duration::from_secs(
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs() - 25 * 3600  // 25 hours ago
);
std::fs::File::create(&old_pdf_path).unwrap();
filetime::set_file_mtime(&old_pdf_path, filetime::FileTime::from_system_time(old_time)).unwrap();
```

**Alternative nếu không muốn thêm crate:** Dùng `#[cfg(unix)]` + `std::process::Command::new("touch").args(["-d", "25 hours ago", path]).status()`. Trên Windows, test 5 sẽ bị skip.

### AC-6: Integration Test — File Lifecycle

**Given** all components implemented
**When** I create integration tests
**Then** add `src-tauri/tests/integration/temp_file_integration_test.rs`:

```rust
use sapo_printer::infrastructure::temp_file::{startup_cleanup, TempPdfFile};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

fn make_test_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sapo_inttest_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_lifecycle_immediate_cleanup() {
    let dir = make_test_dir();
    let pdf_path = dir.join("test_job.pdf");
    fs::write(&pdf_path, b"%PDF-1.4 test").unwrap();

    assert!(pdf_path.exists());
    {
        let _temp = TempPdfFile::new(pdf_path.clone());
        // keep_on_drop = false by default
    } // Dropped here
    assert!(!pdf_path.exists(), "File should be deleted on drop");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_lifecycle_deferred_cleanup() {
    let dir = make_test_dir();
    let pdf_path = dir.join("failed_job.pdf");
    fs::write(&pdf_path, b"%PDF-1.4 test").unwrap();

    {
        let mut temp = TempPdfFile::new(pdf_path.clone());
        temp.keep(); // Job FAILED — keep for debugging
    } // Dropped here
    assert!(pdf_path.exists(), "File should survive drop when keep=true");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_startup_cleanup_full_scenario() {
    let dir = make_test_dir();

    // Create orphaned .tmp file (should always be deleted)
    let tmp_file = dir.join("orphan.tmp");
    fs::write(&tmp_file, b"incomplete download").unwrap();

    // Create recent .pdf file (should be kept)
    let recent_pdf = dir.join("recent.pdf");
    fs::write(&recent_pdf, b"%PDF-1.4").unwrap();

    startup_cleanup(&dir);

    assert!(!tmp_file.exists(), ".tmp files must be deleted on startup");
    assert!(recent_pdf.exists(), "Recent .pdf must be kept");

    let _ = fs::remove_dir_all(&dir);
}
```

Register in `src-tauri/tests/integration/mod.rs` — **append** (follow existing pattern of `mod downloader_integration_test;`):
```rust
// Existing lines in mod.rs:
// mod downloader_integration_test;
// mod renderer_integration_test;
// ADD:
mod temp_file_integration_test;
```

**IMPORTANT:** `src-tauri/tests/integration_tests.rs` (entrypoint) chỉ có `mod integration;` — không cần sửa. Chỉ cần thêm `mod temp_file_integration_test;` vào `tests/integration/mod.rs`.

### AC-7: All Tests Pass

**Given** all components implemented (AC-1 through AC-6)
**When** I run the full test suite
**Then:**
- `cargo test` — all unit tests pass
- `cargo test --test integration_tests` — integration tests pass
- `cargo clippy -- -D warnings` — no clippy warnings
- `cargo fmt --check` — code is formatted

## Tasks / Subtasks

- [x] Task 1: Add `filetime` to `[dev-dependencies]` in `Cargo.toml`
  - [x] Append `filetime = "0.2"` under `[dev-dependencies]` section (hiện chỉ có `mockito = "1.0"`)
- [x] Task 2: Create `src-tauri/src/infrastructure/temp_file.rs` (AC-1, AC-2)
  - [x] Define `TempPdfFile` struct with `path: PathBuf`, `keep_on_drop: bool`
  - [x] Implement `new()`, `path()`, `keep()`, `release()` methods
  - [x] Implement `Drop` trait — `NotFound` silent, other errors `tracing::warn!`, no panic
  - [x] Define `pub(crate) const DEFERRED_RETENTION: Duration`
  - [x] Implement private `fn is_older_than_24h(path: &Path) -> bool`
  - [x] Implement `pub fn startup_cleanup(temp_dir: &Path)` — skip subdirs, best-effort
- [x] Task 3: Register module (AC-3)
  - [x] Add `pub mod temp_file;` to `src-tauri/src/infrastructure/mod.rs`
- [x] Task 4: Update `main.rs` (AC-4)
  - [x] Add `temp_dir` creation after `data_dir` block
  - [x] Call `startup_cleanup(&temp_dir)` before `DbPool::new()`
- [x] Task 5: Write inline unit tests (AC-5)
  - [x] All 10 test cases (gồm Send test và release() test)
- [x] Task 6: Write integration tests (AC-6)
  - [x] Create `src-tauri/tests/integration/temp_file_integration_test.rs`
  - [x] Add `mod temp_file_integration_test;` to `src-tauri/tests/integration/mod.rs`
- [x] Task 7: Final verification (AC-7)
  - [x] `cargo test` passes
  - [x] `cargo clippy -- -D warnings` clean
  - [x] `cargo fmt --check` passes

## Dev Notes

### File Structure

```
src-tauri/src/infrastructure/
├── mod.rs                    # UPDATE: add `pub mod temp_file;`
├── temp_file.rs              # NEW: TempPdfFile struct + startup_cleanup()
└── ... (other modules unchanged)

src-tauri/src/
└── main.rs                   # UPDATE: add temp_dir + startup_cleanup() call

src-tauri/tests/integration/
└── temp_file_integration_test.rs  # NEW: integration tests
```

### Key Design Decisions

**Why `keep_on_drop = false` as default?**
- Success is the common case — immediate cleanup is the default behavior
- Caller must explicitly call `.keep()` for failed jobs to opt into deferred cleanup
- This prevents disk accumulation bugs where developer forgets to cleanup

**Why `startup_cleanup()` takes `&Path` instead of computing it internally?**
- Testability: tests pass their own isolated temp dir
- Flexibility: caller controls the path (same pattern as data_dir creation in `main.rs`)

**Why NOT use `tempfile` crate?**
- We need named files at predictable paths (`~/.sapo-printer/temp/{job_id}.pdf`)
- `ReqwestDownloader` already creates files there — `TempPdfFile` wraps existing paths
- `tempfile` crate generates random names; we need job_id-based names for tracing

**Why `startup_cleanup` deletes ALL `.tmp` regardless of age?**
- `.tmp` files are incomplete downloads — they are NEVER valid for use
- No reason to keep them; any incomplete `.tmp` means the download failed
- Safe to always delete on startup

**Why deferred `.pdf` files kept for 24h?**
- FR-1.4: retry logic may still need the downloaded file for re-submission
- Developers/ops can inspect failed job files within 24h
- 24h is configurable via `app_settings` table (key: `temp_file_retention_hours`) — but for this story, hardcode to 24h

### Codebase Patterns to Follow

**Error handling pattern** (from `reqwest_downloader.rs`):
```rust
// Home directory resolution — same pattern as downloader
let home = home::home_dir().ok_or_else(|| {
    InfrastructureError::ValidationError("Cannot resolve home directory".into())
})?;
```

**Logging pattern** (already used in `circuit_breaker.rs` and printer managers):
```rust
tracing::warn!("...");
tracing::info!("...");
```

**`#[cfg(test)]` inline test pattern** (same as all other infrastructure files):
```rust
#[cfg(test)]
mod tests {
    use super::*;
    // tests here
}
```

**main.rs pattern** (follow existing block style):
```rust
// Lines 192-196: data_dir creation
std::fs::create_dir_all(&data_dir).unwrap_or_else(|e| {
    eprintln!("Cannot create data directory: {e}");
    std::process::exit(1);
});
// Add temp_dir AFTER this block, BEFORE DbPool::new()
```

### Modified Time Handling for Tests

**Option A (preferred):** Add `filetime` to `[dev-dependencies]` in `Cargo.toml`:
```toml
[dev-dependencies]
filetime = "0.2"
```

**Option B:** Use `#[cfg(unix)]` gate for the time-manipulation test and use `touch -A -t` command.

**Option C:** Accept that test 5 (`test_startup_cleanup_removes_old_pdf`) only runs on Unix.

Recommendation: Option A — `filetime` is a small crate (no extra compile deps) and makes tests cross-platform.

### Duration Calculation for Startup Cleanup

```rust
use std::time::{Duration, SystemTime};

// pub(crate) để inline unit tests có thể reference thay vì hardcode 24*3600
pub(crate) const DEFERRED_RETENTION: Duration = Duration::from_secs(24 * 60 * 60);

fn is_older_than_24h(path: &Path) -> bool {
    path.metadata()
        .and_then(|m| m.modified())
        .map(|modified| {
            SystemTime::now()
                .duration_since(modified)
                .map(|age| age > DEFERRED_RETENTION)
                .unwrap_or(true) // If clock went backward, treat as old
        })
        .unwrap_or(true) // If metadata fails, treat as old (safe default: delete)
}
```

**Trong unit tests**, dùng `DEFERRED_RETENTION` thay vì hardcode `25 * 3600`:
```rust
// test 5 — set modified time to just beyond retention threshold:
let threshold_secs = DEFERRED_RETENTION.as_secs() + 3600; // 25h ago
```

### ReqwestDownloader Integration (Future Story 3.5 Queue Worker)

Story 3.5 (Queue Worker) will wrap the downloader's returned `PathBuf` in a `TempPdfFile`:
```rust
// Future Queue Worker pattern (NOT this story's responsibility):
let pdf_path = downloader.download(url, job_id)?;
let mut temp_file = TempPdfFile::new(pdf_path);
// ... attempt to print ...
if print_failed {
    temp_file.keep(); // Keep for 24h debugging
}
// temp_file drops here — immediate if success, kept if failure
```

**This story does NOT wire the downloader — only defines the `TempPdfFile` abstraction.**

### What This Story Does NOT Do

- ❌ Does NOT modify `ReqwestDownloader` — downloader still returns raw `PathBuf`
- ❌ Does NOT wire into queue worker / print job lifecycle — that's `3-5-implement-queue-worker-with-batch-processing` (separate story in sprint)
- ❌ Does NOT implement configurable retention from `app_settings` — hardcode 24h
- ❌ Does NOT modify `AppContext` — `TempPdfFile` is a value type, not a service
- ❌ Does NOT add Tauri commands — purely infrastructure layer

### Previous Story Learnings

**From Story 3.4 (Strategy Selector):**
- `Arc<dyn Trait>` used for shared ownership — `TempPdfFile` is a value type (no Arc needed)
- `Mutex<HashMap>` pattern for thread-safe state — not needed here (TempPdfFile is single-owner)
- Mock pattern in inline tests — use real filesystem in temp_file tests (RAII requires real files)

**From Story 3.3 (Direct PDF Renderer):**
- `validate_pdf_header` duplicated in direct_pdf_renderer to avoid cross-module coupling — same principle: `temp_file.rs` is standalone, no deps on downloader module
- `#[doc(hidden)]` for test-only constructors — use standard `#[cfg(test)]` approach

**From Story 3.1 (Downloader):**
- **CRITICAL:** Temp file path format: `~/.sapo-printer/temp/{job_id}.{ext}` — `TempPdfFile` MUST handle files at this exact location
- Atomic rename `.tmp` → `.pdf` already done in downloader — `TempPdfFile` only wraps `.pdf` files
- `home::home_dir()` is the correct way to get home directory on all platforms

**From Story 2.1 (Database):**
- `main.rs` pattern: create dirs with `unwrap_or_else` + `eprintln!` + `process::exit(1)`
- Add `temp_dir` creation following this exact same pattern

### Cargo.toml Additions

`filetime` **KHÔNG có** trong `Cargo.toml` hiện tại. Cần thêm vào `[dev-dependencies]`:

**File:** `src-tauri/Cargo.toml` — section `[dev-dependencies]` (hiện chỉ có `mockito = "1.0"`):
```toml
[dev-dependencies]
mockito = "1.0"       # existing
filetime = "0.2"      # ADD: for setting file modified time in tests
```

No changes to `[dependencies]` needed. `TempPdfFile` runtime code uses only `std::fs`, `std::path`, `tracing` — all already present.

### References

- [Source: `epics.md` — Story 3.5 Acceptance Criteria (AR-7)]
- [Source: `architecture.md` — AR-7: RAII Pattern for Resource Cleanup]
- [Source: `src-tauri/src/infrastructure/downloader/reqwest_downloader.rs` — temp file path pattern + home crate usage]
- [Source: `src-tauri/src/main.rs` — data_dir creation pattern + where to insert startup_cleanup call]
- [Source: `src-tauri/src/infrastructure/mod.rs` — where to add pub mod temp_file]
- [Source: `src-tauri/src/shared/errors/infrastructure_error.rs` — InfrastructureError variants]

## Dev Agent Record

### Agent Model Used

Qwen Code

### Debug Log References

- `release()` method: cannot move `self.path` out of `TempPdfFile` which implements `Drop`. Fixed by using `self.path.clone()` — Drop runs but `keep_on_drop = true` prevents deletion.

### Completion Notes List

- Implemented `TempPdfFile` RAII wrapper with `Drop` trait for automatic temp PDF cleanup
- Tiered cleanup: immediate (Drop with keep_on_drop=false), deferred (keep() survives Drop), startup (cleanup orphaned files >24h)
- `startup_cleanup()` handles .tmp (always delete), .pdf (delete if >24h old), skips subdirectories
- `main.rs` integration: creates `~/.sapo-printer/temp/` dir and runs startup cleanup before DB init
- 10 inline unit tests covering all RAII behaviors, Send trait, startup cleanup scenarios
- 3 integration tests covering full lifecycle (immediate, deferred, mixed startup cleanup)
- All tests pass, clippy clean, fmt clean

### File List

- `src-tauri/Cargo.toml` — added `filetime = "0.2"` to `[dev-dependencies]`
- `src-tauri/src/infrastructure/temp_file.rs` — NEW: TempPdfFile struct + startup_cleanup() + 10 inline tests
- `src-tauri/src/infrastructure/mod.rs` — added `pub mod temp_file;`
- `src-tauri/src/main.rs` — added temp_dir creation + startup_cleanup() call before DbPool::new()
- `src-tauri/tests/integration/temp_file_integration_test.rs` — NEW: 3 integration tests
- `src-tauri/tests/integration/mod.rs` — added `mod temp_file_integration_test;`

### Change Log

- 2026-06-23: Implemented Story 3.5 — RAII temp file management with tiered cleanup (all 7 tasks complete)

### Review Findings

- [x] [Review][Decision] TempPdfFile không validate path — **Resolved:** Thêm `try_new(path, temp_dir) -> Result` với path validation. Giữ `new()` cho backward compatibility.
- [x] [Review][Decision] Mid-session cleanup cho long-running sessions — **Resolved:** Deferred sang story khác (queue worker/monitoring).
- [x] [Review][Patch] Clock skew gây xóa nhầm file gần đây [src-tauri/src/infrastructure/temp_file.rs:121] — Inner `unwrap_or(true)` trong `is_older_than()` treats future-dated files (clock skew backward) là "old". `duration_since()` returns `Err` khi `now < modified`, map thành `true` → file bị xóa. Fix: đổi inner `unwrap_or(true)` thành `unwrap_or(false)` — khi không xác định được tuổi file, giữ lại an toàn hơn.
- [x] [Review][Defer] Subdirectory cleanup không recursive [src-tauri/src/infrastructure/temp_file.rs:80] — deferred, pre-existing
- [x] [Review][Defer] Không có protection chống 2 TempPdfFile cùng wrap 1 path [src-tauri/src/infrastructure/temp_file.rs:21] — deferred, latent risk
