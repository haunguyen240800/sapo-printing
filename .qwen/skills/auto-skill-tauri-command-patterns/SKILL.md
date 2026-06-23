---
name: tauri-command-patterns
description: Tauri command architecture patterns for this project — lib vs bin crate boundaries, AppContextState placement, and repository trait bounds
source: auto-skill
extracted_at: '2026-06-23T16:40:00.000Z'
---

# Tauri Command Architecture Patterns

Learned from implementing story 3-3 (CreatePrintJobUseCase). These patterns are specific to this project's lib + bin crate split.

## 1. #[tauri::command] cannot cross the lib → main.rs boundary

Tauri's `#[tauri::command]` macro generates internal helper macros (e.g. `__cmd__function_name`) that are scoped to the crate they're defined in. If you define a `#[tauri::command]` function in `src/lib.rs` (the library crate), the generated macros are NOT accessible from `src/main.rs` (the binary crate), even if you `use` the function.

**WRONG** — fails to compile (`cannot find macro __cmd__create_print_job`):

```rust
// src/interface/tauri/commands/print_job.rs (lib crate)
#[tauri::command]
pub fn create_print_job(...) -> Result<...> { ... }

// src/main.rs (bin crate)
use sapo_printer::interface::tauri::commands::print_job::create_print_job;

tauri::generate_handler![create_print_job]  // ERROR: macro not found
```

**RIGHT** — define a plain helper in lib, wrap with `#[tauri::command]` in main.rs:

```rust
// src/interface/tauri/commands/print_job.rs (lib crate)
#[derive(serde::Deserialize)]
pub struct CreateJobPayload { ... }

pub fn execute_create_print_job(
    payload: CreateJobPayload,
    ctx: &AppContextState,
) -> Result<Vec<String>, String> {
    // ... actual logic ...
}

// src/main.rs (bin crate)
#[tauri::command]
fn create_print_job(
    payload: sapo_printer::interface::tauri::commands::print_job::CreateJobPayload,
    ctx: tauri::State<'_, AppContextState>,
) -> Result<Vec<String>, String> {
    sapo_printer::interface::tauri::commands::print_job::execute_create_print_job(
        payload,
        ctx.inner(),  // NOTE: no & — ctx.inner() already returns &T
    )
}
```

**Rule:** All `#[tauri::command]` functions for this project must be defined in `main.rs`. Put shared types (payloads, helpers) in the lib crate and call them from the `main.rs` command wrapper.

## 2. AppContextState must live in lib.rs

`tauri::State<'_, T>` requires `T` to be visible from both `main.rs` (where `.manage(T)` is called) and command functions (where `State<'_, T>` appears in signatures). If `AppContextState` is defined only in `main.rs`, lib crate commands can't reference it.

**RIGHT** — define `AppContextState` in `src/lib.rs`:

```rust
// src/lib.rs
pub struct AppContextState {
    pub printer_repo: Arc<dyn domain::printer::PrinterRepository>,
    pub printer_manager: Arc<dyn infrastructure::printer::PrinterManager>,
    pub _secret_manager: Arc<dyn infrastructure::secrets::SecretManager>,
    pub job_repo: Arc<dyn domain::print_job::PrintJobRepository>,
    pub event_store: Arc<infrastructure::database::SqliteEventStore>,
    pub event_bus: Arc<dyn shared::event_bus::EventBus>,
}
```

Then import in `main.rs`:

```rust
use sapo_printer::AppContextState;
```

**Do NOT** define `AppContextState` in `main.rs` — lib crate code won't be able to use `State<'_, AppContextState>`.

## 3. Repository traits must have Send + Sync bounds

`tauri::State<'_, T>` requires `T: Send + Sync`. If `AppContextState` contains `Arc<dyn SomeTrait>` and the trait lacks `Send + Sync` bounds, you get:

```
error[E0277]: `(dyn SomeTrait + 'static)` cannot be shared between threads safely
```

**Check:** Every trait stored in `AppContextState` must have `Send + Sync`:

```rust
// ✅ Already correct (from printer repository)
pub trait PrinterRepository: Send + Sync { ... }

// ❌ Missing bounds (found in print_job repository — fixed during story 3-3)
pub trait PrintJobRepository: Send + Sync { ... }  // was: pub trait PrintJobRepository { ... }
```

## 4. Database table names — use the actual schema names

This project's migrations define these tables:
- `printer_configs` — stores Printer aggregates (paper size, margins, status, etc.)
- `print_jobs` — stores PrintJob aggregates
- `events` — stores domain events
- `app_settings` — application configuration

**Common mistake:** Inserting into `printers` instead of `printer_configs` in tests. The `PrinterRepository` saves to `printer_configs`, not a table called `printers`.

**Correct test INSERT for a printer:**

```rust
conn.execute(
    "INSERT INTO printer_configs (printer_name, device_id, printer_type, status, created_at, updated_at) \
     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    rusqlite::params![name, name, "Local", "Online", now, now],
)
.unwrap();
```

**Correct test UPDATE for printer status:**

```rust
c.execute(
    "UPDATE printer_configs SET status = 'Online' WHERE printer_name = ?1",
    [name],
)
.unwrap();
```

## 5. ctx.inner() returns &T — no extra & needed

When calling a lib function from a main.rs command that expects `&AppContextState`:

```rust
// WRONG — clippy needless_borrow warning
execute_create_print_job(payload, &ctx.inner())

// RIGHT
execute_create_print_job(payload, ctx.inner())
```

`tauri::State::inner()` already returns `&T`. Adding `&` creates `&&T`.
