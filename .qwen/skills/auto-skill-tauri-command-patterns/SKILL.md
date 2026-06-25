---
name: tauri-command-patterns
description: Tauri command architecture patterns for this project — lib vs bin crate boundaries, AppContextState placement, and repository trait bounds
source: auto-skill
extracted_at: '2026-06-25T10:03:10.861Z'
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
    pub printer_manager: Arc<dyn infrastructure::printer::PrinterManager>,
    pub secret_manager: Arc<dyn infrastructure::secrets::SecretManager>,
    pub job_repo: Arc<dyn domain::print_job::PrintJobRepository>,
    pub event_store: Arc<infrastructure::database::SqliteEventStore>,
    pub event_bus: Arc<dyn shared::event_bus::EventBus>,
    pub queue_manager: Arc<dyn infrastructure::queue::QueueManager>,
    pub queue_worker: Arc<infrastructure::queue::QueueWorker>,
    pub metrics_collector: Arc<infrastructure::metrics::MetricsCollector>,
    pub app_handle: tauri::AppHandle,
    pub install_guard: infrastructure::updater::update_checker::InstallGuard,
    pub last_emitted_update_version: std::sync::Mutex<Option<String>>,
}
```

**Note:** Printer configuration is now stored via `config_store` (JSON file), not a database repository. The `printer_manager` handles OS-level printer discovery and status queries.

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
// ✅ Already correct
pub trait PrintJobRepository: Send + Sync { ... }
pub trait PrinterManager: Send + Sync { ... }

// ❌ Missing bounds (found in print_job repository — fixed during story 3-3)
pub trait PrintJobRepository { ... }  // was missing Send + Sync
```

## 4. Database table names — use the actual schema names

This project's migrations define these tables:
- `print_jobs` — stores PrintJob aggregates
- `events` — stores domain events
- `printer_configs` — legacy table (no longer used by application code; printer config is now stored via `config_store` JSON file)

**Note:** The `PrinterRepository` and `SqlitePrinterRepository` were removed. Printer discovery and status queries now go through `PrinterManager` (OS-level API). Printer configuration is persisted to `~/.sapo-printer/config.json` via `config_store`.

## 5. ctx.inner() returns &T — no extra & needed

When calling a lib function from a main.rs command that expects `&AppContextState`:

```rust
// WRONG — clippy needless_borrow warning
execute_create_print_job(payload, &ctx.inner())

// RIGHT
execute_create_print_job(payload, ctx.inner())
```

`tauri::State::inner()` already returns `&T`. Adding `&` creates `&&T`.

## 6. Commands accessing Tauri plugins — use `AppHandle` directly, not `AppContextState`

When a command needs to access a Tauri plugin (e.g., updater, dialog, notification), take `AppHandle` as a parameter instead of `AppContextState`. Plugins are accessed via extension traits on `AppHandle`, not via managed state.

**RIGHT** — plugin command pattern:

```rust
use tauri_plugin_updater::UpdaterExt;

#[tauri::command]
async fn check_for_updates(
    app: tauri::AppHandle,
) -> Result<UpdateCheckResponse, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater.check().await.map_err(|e| e.to_string())?;
    // ...
}
```

**WRONG** — trying to use AppContextState for plugin access:

```rust
#[tauri::command]
fn check_for_updates(
    ctx: tauri::State<'_, AppContextState>,
) -> Result<UpdateCheckResponse, String> {
    // ❌ AppContextState doesn't have updater access
    // Plugins are accessed via AppHandle extension traits
}
```

**When to use which:**
- `AppContextState` — for commands that need domain services, repositories, infrastructure (print jobs, printers, metrics, etc.)
- `AppHandle` — for commands that need Tauri plugins (updater, dialog, notification, etc.)

**Note:** `AppHandle` is already in `AppContextState` if you need both. But for pure plugin commands, take `AppHandle` directly to keep the signature clean.
