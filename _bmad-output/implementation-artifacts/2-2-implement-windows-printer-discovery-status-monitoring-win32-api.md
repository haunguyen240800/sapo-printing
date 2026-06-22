---
baseline_commit: c1ebff2
---

# Story 2.2: Implement Windows Printer Discovery & Status Monitoring (Win32 API)

Status: done

## Story

As a **nhân viên kho on Windows**,
I want **the app to automatically discover all connected printers and monitor their status**,
So that **I can see which printers are available and their current state**.

## Acceptance Criteria

**Given** the app is running on Windows
**When** I implement Windows printer infrastructure

**AC-1: Traits — `PrinterManager` and `PrinterEngine`**
- `src-tauri/src/infrastructure/printer/printer_manager.rs` must define:
  ```rust
  pub trait PrinterManager: Send + Sync {
      fn discover_printers(&self) -> Vec<Printer>;
      fn get_status(&self, name: &str) -> PrinterStatus;
  }
  ```
- `src-tauri/src/infrastructure/printer/printer_engine.rs` must define:
  ```rust
  pub trait PrinterEngine: Send + Sync {
      fn print(&self, printer_name: &str, data: &[u8]) -> Result<(), InfrastructureError>;
  }
  ```

**AC-2: `Win32PrinterManager` — `discover_printers()`**
- `src-tauri/src/infrastructure/printer/windows/win32_printer_manager.rs` must exist and be gated with `#[cfg(target_os = "windows")]`
- `Win32PrinterManager::discover_printers()` calls `EnumPrintersW` with `PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS` flag at level 2 (`PRINTER_INFO_2W`)
- Returns `Vec<Printer>` constructed from domain `Printer::new()` with appropriate `PrinterName` and `PrinterType`
- `printer_type`: `PrinterType::Network` if `PRINTER_INFO_2W.Attributes` has `PRINTER_ATTRIBUTE_NETWORK` bit set, else `PrinterType::Local`
- When no printers are found or `EnumPrintersW` returns empty, returns empty `Vec` (NOT an error)
- Unsafe block is contained within a private helper function

**AC-3: `Win32PrinterManager` — `get_status()`**
- `get_status(name: &str) -> PrinterStatus` calls `OpenPrinterW` then `GetPrinterW` at level 2 to read `PRINTER_INFO_2W.Status`
- Win32 status → `PrinterStatus` mapping:
  - `0` (no bits set) → `PrinterStatus::Online`
  - `PRINTER_STATUS_OFFLINE` (`0x00000080`) → `PrinterStatus::Offline`
  - Any of `PRINTER_STATUS_ERROR | PRINTER_STATUS_PAPER_JAM | PRINTER_STATUS_PAPER_OUT | PRINTER_STATUS_OUTPUT_BIN_FULL | PRINTER_STATUS_NO_TONER` → `PrinterStatus::Error`
  - `OpenPrinterW` failure (printer not found) → `PrinterStatus::Offline`
  - Any unrecognized status bits → `PrinterStatus::Online` (safe default)
- `ClosePrinter` is always called after `OpenPrinterW` succeeds (no leaks)

**AC-4: `WindowsPrinterEngine` — stub**
- `src-tauri/src/infrastructure/printer/windows/windows_printer_engine.rs` must exist, gated with `#[cfg(target_os = "windows")]`
- Implement `PrinterEngine` trait — `print()` returns `Ok(())` unconditionally (stub for now)

**AC-5: Cargo.toml — Windows features**
- Add `Win32_Graphics_Printing` feature to the windows dependency:
  ```toml
  [target.'cfg(windows)'.dependencies]
  windows = { version = "0.52", features = ["Security_Credentials", "Win32_Graphics_Printing"] }
  ```
- No new crates added

**AC-6: Module wiring**
- `src-tauri/src/infrastructure/printer/mod.rs` must declare:
  - `pub mod printer_manager;`
  - `pub mod printer_engine;`
  - `#[cfg(target_os = "windows")] pub mod windows;`
  - Re-export: `pub use printer_manager::PrinterManager;`
  - Re-export: `pub use printer_engine::PrinterEngine;`
- `src-tauri/src/infrastructure/printer/windows/mod.rs` must declare:
  - `#[cfg(target_os = "windows")] pub mod win32_printer_manager;`
  - `#[cfg(target_os = "windows")] pub mod windows_printer_engine;`
  - Re-export: `pub use win32_printer_manager::Win32PrinterManager;`
  - Re-export: `pub use windows_printer_engine::WindowsPrinterEngine;`

**AC-7: `InfrastructureError` type**
- `src-tauri/src/shared/errors/infrastructure_error.rs` must be created with:
  ```rust
  #[derive(Debug)]
  pub enum InfrastructureError {
      PrinterError { reason: String },
  }
  impl std::fmt::Display + std::error::Error for InfrastructureError
  ```
- `src-tauri/src/shared/errors/mod.rs` must declare `pub mod infrastructure_error;` and re-export `InfrastructureError`
- `src-tauri/src/shared/mod.rs` must declare `pub mod errors;`

**AC-8: Unit Tests**
- `win32_printer_manager.rs` inline tests (`#[cfg(test)]`):
  - `test_status_mapping_zero_is_online` — call status mapper fn with `0u32` → `PrinterStatus::Online`
  - `test_status_mapping_offline_flag` — call with `0x00000080u32` → `PrinterStatus::Offline`
  - `test_status_mapping_error_flag` — call with `PRINTER_STATUS_ERROR` value → `PrinterStatus::Error`
  - `test_discover_returns_vec` — `Win32PrinterManager::new().discover_printers()` returns a `Vec<Printer>` without panicking (may be empty in CI)
- `windows_printer_engine.rs` inline tests:
  - `test_print_stub_returns_ok` — `WindowsPrinterEngine::new().print("any", &[])` returns `Ok(())`

**AC-9: All Tests Pass**
- `cargo test` — all tests pass, no regressions (101+ existing tests)
- `cargo build` — zero errors
- `cargo clippy` — zero warnings

## Tasks / Subtasks

- [x] **Task 1: `InfrastructureError`** (AC: #7)
  - [x] Create `src-tauri/src/shared/errors/` directory with `mod.rs` and `infrastructure_error.rs`
  - [x] Define `InfrastructureError` enum with `PrinterError { reason: String }`
  - [x] Implement `Display` + `std::error::Error`
  - [x] Update `src-tauri/src/shared/mod.rs` — add `pub mod errors;`

- [x] **Task 2: Trait files** (AC: #1)
  - [x] Create `src-tauri/src/infrastructure/printer/printer_manager.rs` — `PrinterManager` trait
  - [x] Create `src-tauri/src/infrastructure/printer/printer_engine.rs` — `PrinterEngine` trait

- [x] **Task 3: Update Cargo.toml** (AC: #5)
  - [x] Add `Win32_Graphics_Printing` to windows features

- [x] **Task 4: `Win32PrinterManager`** (AC: #2, #3)
  - [x] Create `src-tauri/src/infrastructure/printer/windows/win32_printer_manager.rs`
  - [x] Implement `discover_printers()` using `EnumPrintersW` level 2
  - [x] Extract private `map_win32_status(status: u32) -> PrinterStatus` helper
  - [x] Implement `get_status()` using `OpenPrinterW` + `GetPrinterW` level 2
  - [x] Ensure `ClosePrinter` called in all `get_status()` paths

- [x] **Task 5: `WindowsPrinterEngine` stub** (AC: #4)
  - [x] Create `src-tauri/src/infrastructure/printer/windows/windows_printer_engine.rs`
  - [x] Implement `PrinterEngine::print()` returning `Ok(())`

- [x] **Task 6: Wire modules** (AC: #6)
  - [x] Replace stub content in `src-tauri/src/infrastructure/printer/mod.rs`
  - [x] Create `src-tauri/src/infrastructure/printer/windows/mod.rs`

- [x] **Task 7: Tests + verify** (AC: #8, #9)
  - [x] Add inline tests to `win32_printer_manager.rs` (status mapping + discover_returns_vec)
  - [x] Add inline test to `windows_printer_engine.rs` (print stub)
  - [x] `cargo build` — zero errors
  - [x] `cargo test` — all pass
  - [x] `cargo clippy` — zero warnings

## Dev Notes

### 🎯 Story Scope

Windows-only printer discovery via Win32 API. No repository persistence (Story 2.4), no UI (Stories 2.5/2.6), no CUPS (Story 2.3).

**New files:**
```
src-tauri/src/infrastructure/printer/printer_manager.rs   (NEW)
src-tauri/src/infrastructure/printer/printer_engine.rs    (NEW)
src-tauri/src/infrastructure/printer/windows/mod.rs       (NEW)
src-tauri/src/infrastructure/printer/windows/win32_printer_manager.rs  (NEW)
src-tauri/src/infrastructure/printer/windows/windows_printer_engine.rs (NEW)
src-tauri/src/shared/errors/mod.rs                        (NEW)
src-tauri/src/shared/errors/infrastructure_error.rs       (NEW)
```

**Modified files:**
```
src-tauri/src/infrastructure/printer/mod.rs   (replace 2-line stub)
src-tauri/src/shared/mod.rs                   (add pub mod errors)
src-tauri/Cargo.toml                          (add Win32_Graphics_Printing feature)
```

**DO NOT touch:** domain layer, database module, main.rs, AppContext, secrets.

### 📦 Win32 API Implementation

The two-call pattern for `EnumPrintersW` (first call gets buffer size, second call gets data):

```rust
use windows::Win32::Graphics::Printing::{
    EnumPrintersW, PRINTER_ENUM_LOCAL, PRINTER_ENUM_CONNECTIONS,
    PRINTER_INFO_2W, PRINTER_ATTRIBUTE_NETWORK,
};
use windows::core::PCWSTR;

fn enum_printers_raw() -> Vec<PRINTER_INFO_2W> {
    unsafe {
        // First call: get needed buffer size
        let mut bytes_needed: u32 = 0;
        let mut count_returned: u32 = 0;
        let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;

        let _ = EnumPrintersW(flags, PCWSTR::null(), 2, None, &mut bytes_needed, &mut count_returned);

        if bytes_needed == 0 {
            return vec![];
        }

        // Second call: fill buffer
        let mut buf: Vec<u8> = vec![0u8; bytes_needed as usize];
        if EnumPrintersW(
            flags,
            PCWSTR::null(),
            2,
            Some(buf.as_mut_slice()),
            &mut bytes_needed,
            &mut count_returned,
        ).is_err() {
            return vec![];
        }

        // Cast buffer to slice of PRINTER_INFO_2W
        let ptr = buf.as_ptr() as *const PRINTER_INFO_2W;
        std::slice::from_raw_parts(ptr, count_returned as usize).to_vec()
    }
}
```

Reading `pPrinterName` from `PRINTER_INFO_2W` (it's a `PWSTR`):

```rust
fn pwstr_to_string(s: windows::core::PWSTR) -> String {
    if s.is_null() { return String::new(); }
    unsafe { s.to_string().unwrap_or_default() }
}
```

`get_status` pattern using `OpenPrinterW` + `GetPrinterW`:

```rust
use windows::Win32::Graphics::Printing::{
    OpenPrinterW, GetPrinterW, ClosePrinter, PRINTER_INFO_2W,
    PRINTER_STATUS_OFFLINE, PRINTER_STATUS_ERROR,
    PRINTER_STATUS_PAPER_JAM, PRINTER_STATUS_PAPER_OUT,
    PRINTER_STATUS_OUTPUT_BIN_FULL, PRINTER_STATUS_NO_TONER,
};
use windows::core::PCWSTR;
use std::mem;

fn query_printer_status(name: &str) -> PrinterStatus {
    unsafe {
        // Encode name as wide string
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut handle = windows::Win32::Graphics::Printing::HANDLE::default();

        if OpenPrinterW(PCWSTR(wide.as_ptr()), &mut handle, None).is_err() {
            return PrinterStatus::Offline;
        }

        // Get size first
        let mut needed: u32 = 0;
        let _ = GetPrinterW(handle, 2, None, &mut needed);

        let mut buf = vec![0u8; needed as usize];
        let ok = GetPrinterW(handle, 2, Some(buf.as_mut_slice()), &mut needed);
        ClosePrinter(handle);  // always close

        if ok.is_err() {
            return PrinterStatus::Offline;
        }

        let info = &*(buf.as_ptr() as *const PRINTER_INFO_2W);
        map_win32_status(info.Status)
    }
}
```

Status mapping helper (extractable for unit testing WITHOUT calling Win32):

```rust
fn map_win32_status(status: u32) -> PrinterStatus {
    const ERROR_FLAGS: u32 =
        PRINTER_STATUS_ERROR.0
        | PRINTER_STATUS_PAPER_JAM.0
        | PRINTER_STATUS_PAPER_OUT.0
        | PRINTER_STATUS_OUTPUT_BIN_FULL.0
        | PRINTER_STATUS_NO_TONER.0;

    if status & PRINTER_STATUS_OFFLINE.0 != 0 {
        PrinterStatus::Offline
    } else if status & ERROR_FLAGS != 0 {
        PrinterStatus::Error
    } else {
        PrinterStatus::Online
    }
}
```

> ⚠️ `PRINTER_STATUS_*` constants in `windows` 0.52 are `PRINTER_STATUS(u32)` newtype wrappers — access `.0` for the raw value.

### 📦 Complete `Win32PrinterManager` struct

```rust
use crate::domain::printer::{Printer, PrinterName, PrinterStatus, PrinterType};
use super::super::printer_manager::PrinterManager;

pub struct Win32PrinterManager;

impl Win32PrinterManager {
    pub fn new() -> Self { Self }
}

impl Default for Win32PrinterManager {
    fn default() -> Self { Self::new() }
}

impl PrinterManager for Win32PrinterManager {
    fn discover_printers(&self) -> Vec<Printer> { ... }
    fn get_status(&self, name: &str) -> PrinterStatus { ... }
}
```

### 📦 Trait Signatures (exact)

```rust
// printer_manager.rs
use crate::domain::printer::{Printer, PrinterStatus};

pub trait PrinterManager: Send + Sync {
    fn discover_printers(&self) -> Vec<Printer>;
    fn get_status(&self, name: &str) -> PrinterStatus;
}

// printer_engine.rs
use crate::shared::errors::InfrastructureError;

pub trait PrinterEngine: Send + Sync {
    fn print(&self, printer_name: &str, data: &[u8]) -> Result<(), InfrastructureError>;
}
```

### ⚠️ Cargo.toml — Correct Feature Merge

Current entry in `Cargo.toml`:
```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.52", features = ["Security_Credentials"] }
```

After update (ADD feature to same entry, do NOT create duplicate):
```toml
[target.'cfg(windows)'.dependencies]
windows = { version = "0.52", features = ["Security_Credentials", "Win32_Graphics_Printing"] }
```

### ⚠️ `cfg` Guard Placement

All Windows-specific code must be gated. The pattern used in this project (see `app_context.rs`):
```rust
#[cfg(target_os = "windows")]
pub mod windows;
```
Non-Windows builds must compile cleanly. The traits (`PrinterManager`, `PrinterEngine`) are platform-agnostic and must NOT be gated.

### ⚠️ `shared/mod.rs` — Existing Content

Current `src-tauri/src/shared/mod.rs` content (from Story 1.4):
```rust
pub mod event_bus;
pub mod app_context;
pub mod utils;
```
Add `pub mod errors;` to this file. Do NOT remove existing entries.

### ⚠️ `infrastructure/printer/mod.rs` — Current Content

Current content is a 2-line comment stub. Replace entirely:
```rust
pub mod printer_manager;
pub mod printer_engine;

#[cfg(target_os = "windows")]
pub mod windows;

pub use printer_manager::PrinterManager;
pub use printer_engine::PrinterEngine;
```

### 🧪 Unit Tests (Status Mapping is Key)

The `map_win32_status` helper must be `pub(crate)` or at minimum accessible to the inline test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::printer::PrinterStatus;

    #[test]
    fn test_status_mapping_zero_is_online() {
        assert_eq!(map_win32_status(0), PrinterStatus::Online);
    }

    #[test]
    fn test_status_mapping_offline_flag() {
        // PRINTER_STATUS_OFFLINE = 0x80
        assert_eq!(map_win32_status(0x00000080), PrinterStatus::Offline);
    }

    #[test]
    fn test_status_mapping_error_flag() {
        // PRINTER_STATUS_ERROR = 0x2
        assert_eq!(map_win32_status(0x00000002), PrinterStatus::Error);
    }

    #[test]
    fn test_discover_returns_vec() {
        // Does not panic — may return empty Vec in CI
        let manager = Win32PrinterManager::new();
        let _printers = manager.discover_printers();
        // No assertion on count — just must not panic
    }
}
```

For `windows_printer_engine.rs`:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_stub_returns_ok() {
        let engine = WindowsPrinterEngine::new();
        assert!(engine.print("any_printer", &[]).is_ok());
    }
}
```

### 🔗 Dependencies on Previous Stories

| Story | What's needed |
|-------|---------------|
| 1.3   | `Printer`, `PrinterName`, `PrinterStatus`, `PrinterType` from `domain::printer` |
| 1.4   | `AppContext::platform_engine_name()` pattern for `#[cfg]` usage reference |
| 2.1   | `InfrastructureError` goes in `shared/errors/` (same layer as `DatabaseError` pattern) |

`InfrastructureError::PrinterError` follows the exact same pattern as `DatabaseError` in story 2.1:
- `Debug` derive
- `Display` + `std::error::Error` manual impl
- `reason: String` field

### 🚫 Anti-Patterns (DO NOT)

1. ❌ DO NOT add a new `[target.'cfg(windows)'.dependencies]` section — merge into existing one
2. ❌ DO NOT use `#[cfg(windows)]` (wrong) — use `#[cfg(target_os = "windows")]` (correct, matches project pattern)
3. ❌ DO NOT implement `PrinterRepository` here — that is Story 2.4
4. ❌ DO NOT call `run_migrations` or touch `DbPool` — database is Story 2.1 (done)
5. ❌ DO NOT add `PrinterCache` — that is deferred to Story 2.4 or later (status polling 5s TTL is Epic 3 concern)
6. ❌ DO NOT `unwrap()` on Win32 results — use `is_err()` checks and return safe defaults
7. ❌ DO NOT `leak` printer handles — `ClosePrinter` must be called in every code path after `OpenPrinterW` succeeds
8. ❌ DO NOT gate trait files with `#[cfg]` — traits are cross-platform contracts

### 📚 Architecture References

- AR-3 Cross-Platform Abstraction: `architecture.md` Decision 5 — trait-based with `#[cfg(target_os)]`
- AR-4 Repository Pattern: `architecture.md` — `PrinterManager` trait is **not** a repository; repository comes in 2.4
- `infrastructure/printer/` structure: `architecture.md` §2510 file tree
- Error handling 3-tier: `architecture.md` §168 — `InfrastructureError` lives in `shared/errors/`
- `PrinterStatus` mapping note: `architecture.md` Decision 18 — `PrinterCache` TTL 5s is a future concern

## Dev Agent Record

### Agent Model Used

Claude Sonnet 4 (via Kiro)

### Debug Log References

- Cargo.toml required additional features beyond `Win32_Graphics_Printing`: `Win32_Foundation`, `Win32_Graphics_Gdi`, `Win32_Security` — needed by `OpenPrinterW`, `GetPrinterW`, `ClosePrinter`, `PRINTER_INFO_2W`.
- `PRINTER_STATUS_*` constants in windows 0.52 are plain `u32` (not newtype wrappers) — `.0` accessor not needed.
- `HANDLE` lives in `Win32::Foundation`, not `Win32::Graphics::Printing`.

### Completion Notes List

- AC-1: `PrinterManager` and `PrinterEngine` traits created in `infrastructure/printer/`.
- AC-2/3: `Win32PrinterManager` implements `discover_printers()` via `EnumPrintersW` level 2 and `get_status()` via `OpenPrinterW`+`GetPrinterW`. `ClosePrinter` called in all paths. `map_win32_status` is a standalone pure fn for testability.
- AC-4: `WindowsPrinterEngine` stub returns `Ok(())` unconditionally.
- AC-5: `Win32_Foundation`, `Win32_Graphics_Gdi`, `Win32_Graphics_Printing`, `Win32_Security` added to windows features.
- AC-6: `infrastructure/printer/mod.rs` replaced; `windows/mod.rs` created with correct `cfg` guards and re-exports.
- AC-7: `InfrastructureError` created in `shared/errors/`; `shared/errors/mod.rs` updated; `shared/mod.rs` already declared `pub mod errors`.
- AC-8/9: 5 new tests added. `cargo test` — 106 passed. `cargo build` — zero errors. `cargo clippy` — zero warnings.

### File List

- `src-tauri/src/shared/errors/infrastructure_error.rs` (NEW)
- `src-tauri/src/shared/errors/mod.rs` (MODIFIED)
- `src-tauri/src/infrastructure/printer/printer_manager.rs` (NEW)
- `src-tauri/src/infrastructure/printer/printer_engine.rs` (NEW)
- `src-tauri/src/infrastructure/printer/mod.rs` (MODIFIED)
- `src-tauri/src/infrastructure/printer/windows/mod.rs` (NEW)
- `src-tauri/src/infrastructure/printer/windows/win32_printer_manager.rs` (NEW)
- `src-tauri/src/infrastructure/printer/windows/windows_printer_engine.rs` (NEW)
- `src-tauri/Cargo.toml` (MODIFIED)


## Change Log

- 2026-06-22: Story 2.2 implemented — Win32 printer discovery/status, traits, InfrastructureError, WindowsPrinterEngine stub. 106 tests pass, zero warnings.

### Review Findings

- [x] [Review][Patch] Re-export thiếu `cfg` guard trong `windows/mod.rs` — `pub use win32_printer_manager::Win32PrinterManager` và `pub use windows_printer_engine::WindowsPrinterEngine` không có `#[cfg(target_os = "windows")]` → compile error trên non-Windows [`src-tauri/src/infrastructure/printer/windows/mod.rs:5-6`]
- [x] [Review][Patch] Use-after-free: `PWSTR` trong `Vec<PRINTER_INFO_2W>` trỏ vào buffer `buf` đã drop sau khi `enum_printers_raw()` return — đọc `pPrinterName` trong `discover_printers()` là UB [`src-tauri/src/infrastructure/printer/windows/win32_printer_manager.rs`]
- [x] [Review][Patch] `query_printer_status`: nếu `GetPrinterW` lần 1 trả về `needed = 0`, lần 2 gọi với empty buffer rồi cast pointer → UB [`src-tauri/src/infrastructure/printer/windows/win32_printer_manager.rs`]
- [x] [Review][Patch] AC-8: `test_print_stub_returns_ok` không có `#[cfg(target_os = "windows")]` guard → fail compile non-Windows vì `PrinterEngine` impl chỉ tồn tại trên Windows [`src-tauri/src/infrastructure/printer/windows/windows_printer_engine.rs`]
- [x] [Review][Patch] AC-2: unsafe inline trong trait impl `discover_printers()` — AC-2 yêu cầu unsafe block nằm trong private helper function [`src-tauri/src/infrastructure/printer/windows/win32_printer_manager.rs`]
- [x] [Review][Defer] `map_win32_status`: combined flags (e.g. `OFFLINE | ERROR`) không có test — pre-existing gap, hành vi đúng nhưng chưa documented [`src-tauri/src/infrastructure/printer/windows/win32_printer_manager.rs`] — deferred, pre-existing
- [x] [Review][Defer] `PrinterName::new("")` khi `pPrinterName` null — behavior phụ thuộc domain validation của `PrinterName` (Story 1.3) [`src-tauri/src/infrastructure/printer/windows/win32_printer_manager.rs`] — deferred, pre-existing
