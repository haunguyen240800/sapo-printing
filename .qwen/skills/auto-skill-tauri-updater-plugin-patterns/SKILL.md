---
name: tauri-updater-plugin-patterns
description: Tauri v2 auto-update plugin (tauri-plugin-updater) integration patterns — registration, capabilities, signing, periodic checks, and command structure
source: auto-skill
extracted_at: '2026-06-25T05:41:33.595Z'
---

# Tauri v2 Updater Plugin Patterns

Learned from creating story 4-6 (auto-update with code signing). These patterns are specific to Tauri v2 and this project's architecture.

## 1. Plugin Registration — use `app.handle().plugin()` inside `.setup()`, NOT `tauri::Builder::default().plugin()`

Tauri v2 changed how plugins are registered. The updater plugin must be registered inside the `.setup()` callback using `app.handle().plugin()`.

**WRONG** — Tauri v1 style or incorrect placement:

```rust
// ❌ Don't chain on Builder
tauri::Builder::default()
    .plugin(tauri_plugin_updater::Builder::new().build())
    .setup(|app| { ... })
    .run(...)
```

**RIGHT** — Tauri v2 pattern:

```rust
tauri::Builder::default()
    .setup(|app| {
        // ✅ Register inside .setup()
        app.handle().plugin(
            tauri_plugin_updater::Builder::new().build()
        )?;

        // ... rest of setup ...
        Ok(())
    })
    .run(...)
```

## 2. UpdaterExt trait — import only where `.updater()` is called

The `UpdaterExt` trait provides `.updater()` and `.updater_builder()` methods on `AppHandle`. Import it **only in the infrastructure module** where you call `.updater()` — NOT in `main.rs` where the plugin is registered.

```rust
// In infrastructure/updater/update_checker.rs:
use tauri_plugin_updater::UpdaterExt;

let updater = app_handle.updater().map_err(|e| e.to_string())?;
let update = updater.check().await.map_err(|e| e.to_string())?;
```

```rust
// In main.rs — NO UpdaterExt import needed:
// Just register the plugin:
app.handle().plugin(
    tauri_plugin_updater::Builder::new().build()
)?;
```

**Without this import in the right place**, you get: `no method named 'updater' found for struct 'AppHandle'`.
**With an unused import in main.rs**, you get a compiler warning.

## 2.1 Update API — `install()` takes bytes, use `download_and_install()` for convenience

The `Update` struct returned by `updater.check()` has three key methods:

**`download()` — async, returns bytes:**
```rust
pub async fn download<C, D>(&self, on_chunk: C, on_finish: D) -> Result<Vec<u8>>
```
Downloads the update package, verifies signature, returns raw bytes.

**`install()` — synchronous, takes bytes:**
```rust
pub fn install(&self, bytes: impl AsRef<[u8]>) -> Result<()>
```
**NOT zero-argument!** You must pass the downloaded bytes. This is synchronous (not async) — it just extracts and runs the installer.

**`download_and_install()` — async, convenience method:**
```rust
pub async fn download_and_install<C, D>(&self, on_chunk: C, on_finish: D) -> Result<()>
```
Combines download + install in one call. **Use this for most cases.**

**WRONG** — calling `install()` with no args:
```rust
// ❌ Compilation error: "this method takes 1 argument but 0 arguments were supplied"
update.install().await  // install() is NOT async and requires bytes!
```

**RIGHT** — use `download_and_install()`:
```rust
// ✅ Correct pattern
update.download_and_install(
    |chunk_length, content_length| {
        tracing::info!(chunk_length, content_length = ?content_length, "Downloading...");
    },
    || {
        tracing::info!("Download finished");
    },
).await.map_err(|e| format!("Update install failed: {}", e))?;
```

**Alternative** — manual download + install:
```rust
let bytes = update.download(
    |chunk, total| { /* progress */ },
    || { /* done */ }
).await?;

update.install(bytes)?;  // synchronous, pass bytes
```

**`Update.version` is `String`** (not `semver::Version`):
```rust
pub version: String,  // e.g., "0.2.0"
pub body: Option<String>,  // release notes
```

## 3. Cargo.toml dependency

```toml
[dependencies]
tauri-plugin-updater = "2"
```

Version `"2"` is compatible with Tauri 2.0.x. Latest stable is 2.10.x.

## 4. tauri.conf.json configuration

Two additions needed:

**a) `createUpdaterArtifacts` in `bundle`:**
```json
{
  "bundle": {
    "active": true,
    "targets": "all",
    "createUpdaterArtifacts": true
  }
}
```
This tells the bundler to generate `.sig` signature files alongside update bundles during `cargo tauri build`.

**b) `plugins.updater` section:**
```json
{
  "plugins": {
    "updater": {
      "pubkey": "<PUBLIC_KEY_PEM_CONTENTS>",
      "endpoints": [
        "https://github.com/<org>/<repo>/releases/latest/download/latest.json"
      ],
      "windows": {
        "installMode": "passive"
      }
    }
  }
}
```

**Critical:** `pubkey` must be the **actual PEM string content** from the `.pub` file, NOT a file path.

## 5. Capabilities — add `"updater:default"` permission

The current `capabilities/default.json` must include the updater permission:

```json
{
  "identifier": "default",
  "description": "Default permissions for the main window",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "updater:default"
  ]
}
```

**Without this**, the frontend cannot call updater-related IPC and commands will fail silently or with permission errors.

## 6. Commands — `check_for_updates` uses `AppHandle`, `install_update` uses `AppContextState` for concurrency guard

The `check_for_updates` command accesses the plugin via `AppHandle.updater()`. The `install_update` command additionally needs `AppContextState` for the `InstallGuard` (prevents concurrent installs) and `last_emitted_update_version` (reset after install).

**RIGHT** — updater command pattern:

```rust
// check_for_updates — only needs AppHandle
#[tauri::command]
async fn check_for_updates(
    app: tauri::AppHandle,
) -> Result<UpdateCheckResponse, String> {
    sapo_printer::interface::tauri::commands::update::execute_check_for_updates(&app).await
}

// install_update — needs both AppHandle AND AppContextState (for InstallGuard + dedup reset)
#[tauri::command]
async fn install_update(
    app: tauri::AppHandle,
    ctx: tauri::State<'_, AppContextState>,
) -> Result<(), String> {
    sapo_printer::interface::tauri::commands::update::execute_install_update(
        &app,
        &ctx.install_guard,
        &ctx.last_emitted_update_version,
    ).await
}
```

**AppContextState** must include `install_guard` and `last_emitted_update_version`:
```rust
pub struct AppContextState {
    // ... other fields ...
    pub install_guard: infrastructure::updater::update_checker::InstallGuard,
    pub last_emitted_update_version: std::sync::Mutex<Option<String>>,
}
```

**InstallGuard** uses RAII pattern for panic-safe concurrency control. The `try_acquire()` method returns `Option<InstallGuardGuard>` — the guard auto-releases on drop (including panic):

```rust
pub struct InstallGuard {
    installing: Arc<AtomicBool>,
}

/// RAII guard that releases InstallGuard on drop (panic-safe).
pub struct InstallGuardGuard<'a> {
    guard: &'a InstallGuard,
}

impl InstallGuard {
    pub fn new() -> Self {
        Self { installing: Arc::new(AtomicBool::new(false)) }
    }

    /// Returns Some(guard) if acquired, None if already installing.
    /// Guard auto-releases on drop.
    pub fn try_acquire(&self) -> Option<InstallGuardGuard<'_>> {
        if self.installing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            Some(InstallGuardGuard { guard: self })
        } else {
            None
        }
    }

    fn release(&self) {
        self.installing.store(false, Ordering::SeqCst);
    }
}

impl<'a> Drop for InstallGuardGuard<'a> {
    fn drop(&mut self) {
        self.guard.release();
    }
}
```

**WRONG** — manual release (not panic-safe):
```rust
// ❌ If download_and_install panics, release() is never called
if !install_guard.try_acquire() { return Err(...); }
let result = download_and_install_update(app).await;
install_guard.release();  // unreachable on panic
result
```

**RIGHT** — RAII guard (panic-safe):
```rust
// ✅ Guard auto-releases on drop, even if .await panics
let _guard = install_guard
    .try_acquire()
    .ok_or_else(|| "An update is already being installed".to_string())?;
download_and_install_update(app).await
```

**Post-install restart flow** — after successful install, emit `"update-ready-to-apply"` event and reset dedup so periodic check re-notifies if user doesn't restart:

```rust
pub async fn execute_install_update(
    app: &AppHandle,
    install_guard: &InstallGuard,
    last_emitted_version: &std::sync::Mutex<Option<String>>,
) -> Result<(), String> {
    let _guard = install_guard
        .try_acquire()
        .ok_or_else(|| "An update is already being installed".to_string())?;

    update_checker::download_and_install_update(app).await?;

    // Reset dedup so periodic check re-notifies if user doesn't restart
    if let Ok(mut version) = last_emitted_version.lock() {
        *version = None;
    }

    // Emit event so frontend can prompt user to restart
    let _ = app.emit("update-ready-to-apply", ());

    Ok(())
}
```

**Why not auto-restart?** `app.restart()` would kill the app immediately, potentially losing user work. Instead, emit an event and let the frontend prompt the user. If the user ignores the prompt, the dedup reset ensures the next periodic check (24h) re-emits `"update-available"`.

## 7. No Use Case needed for thin plugin wrappers

Unlike metrics (which aggregates SQL data), the update check is a single API call to the plugin. Adding a `CheckForUpdatesUseCase` in the application layer would be over-engineering.

**RIGHT** — command handler calls infrastructure directly:

```rust
// commands/update.rs (interface layer)
pub async fn execute_check_for_updates(app: AppHandle) -> Result<UpdateCheckResponse, String> {
    let result = infrastructure::updater::update_checker::check_for_updates(&app).await?;
    Ok(UpdateCheckResponse { ... })
}
```

**Rule:** If the operation is a thin pass-through to a plugin/external API with no business logic, aggregation, or transactions, skip the use case layer.

## 8. Periodic update check — spawn in `.setup()` with `tauri::async_runtime::spawn()`

The plugin has no built-in periodic check. Implement it yourself:

```rust
// Inside .setup(), after plugin registration:
let update_handle = app.handle().clone();
tauri::async_runtime::spawn(async move {
    // Startup check (immediate)
    match check_for_updates(&update_handle).await {
        Ok(result) if result.update_available => {
            let _ = update_handle.emit("update-available", &result);
        }
        Ok(_) => {}
        Err(e) => tracing::warn!(error = %e, "Startup update check failed"),
    }

    // Periodic check every 24 hours
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(24 * 60 * 60)).await;
        match check_for_updates(&update_handle).await {
            Ok(result) if result.update_available => {
                let _ = update_handle.emit("update-available", &result);
            }
            Ok(_) => {}
            Err(e) => tracing::warn!(error = %e, "Periodic update check failed"),
        }
    }
});
```

**Critical:** Always use `tauri::async_runtime::spawn()` — never block `.setup()` with async work.

## 9. Code signing — mandatory, cannot be disabled

**Key generation:**
```bash
cargo tauri signer generate -w ~/.tauri/sapo-printer.key
```

**Build-time env vars (PowerShell):**
```powershell
$env:TAURI_SIGNING_PRIVATE_KEY="path/to/sapo-printer.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
```

**Build-time env vars (Linux/macOS):**
```bash
export TAURI_SIGNING_PRIVATE_KEY="path/to/sapo-printer.key"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
```

**Public key:** Read the `.pub` file contents and paste into `tauri.conf.json` → `plugins.updater.pubkey`.

## 10. latest.json manifest format

For GitHub Releases, the endpoint must return:

```json
{
  "version": "0.2.0",
  "notes": "Bug fixes and improvements",
  "pub_date": "2026-06-25T00:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "RAW_CONTENT_OF_.SIG_FILE",
      "url": "https://github.com/org/repo/releases/download/v0.2.0/app_x64.msi.zip"
    }
  }
}
```

**Critical:** The `signature` field must be the **raw string content** of the `.sig` file, not a URL or file path. The `.sig` files are generated automatically by `cargo tauri build` when `createUpdaterArtifacts: true`.

## 11. Native Messaging mode — no updater needed

The `run_native_messaging_mode()` function is headless (no UI). Auto-update only makes sense in the interactive Tauri mode. Do NOT add updater initialization to native messaging mode.

## 12. Emitting update-available event to frontend

When an update is detected, emit a Tauri event for the frontend popup (story 4-7). **Always emit the DTO type** (`UpdateCheckResponse`), never the infrastructure type (`UpdateCheckResult`), to maintain a stable frontend contract.

**Dedup state must be shared** — store `last_emitted_version` in `AppContextState` (as `Mutex<Option<String>>`) so both the background task and the install command can access it. The install command resets it to `None` after a successful install, ensuring the periodic check re-notifies if the user doesn't restart.

```rust
// Inside the spawned background task:
use sapo_printer::interface::tauri::dtos::update::UpdateCheckResponse;
use tauri::Emitter;

// After check_for_updates returns Ok(result) with update_available:
let state = update_handle.state::<sapo_printer::AppContextState>();
let mut last_emitted = state.last_emitted_update_version.lock().unwrap();
if result.version != *last_emitted {
    let dto = UpdateCheckResponse {
        update_available: result.update_available,
        version: result.version.clone(),
        release_notes: result.release_notes,
    };
    let _ = update_handle.emit("update-available", &dto);
    *last_emitted = result.version;
}
```

**WRONG** — local variable for dedup (install command can't reset it):
```rust
// ❌ last_emitted_version is local to the spawned task
let mut last_emitted_version: Option<String> = None;
// After install, this is still the old value — periodic check won't re-emit
```

**WRONG** — emitting infrastructure type directly:
```rust
// ❌ Couples frontend to infrastructure layer
let _ = update_handle.emit("update-available", &result); // result is UpdateCheckResult
```

Frontend listens for two events:
```typescript
import { listen } from '@tauri-apps/api/event';

// Update available — show popup with version info
listen('update-available', (event) => { /* show popup */ });

// Update ready to apply — prompt user to restart
listen('update-ready-to-apply', () => { /* prompt restart */ });
```

## 13. Pitfalls and Gotchas (from code review)

These issues were discovered during code review and should be avoided in future implementations:

### 13.1 TOCTOU gap between check and install

**Problem:** `download_and_install_update()` calls `updater.check()` internally, creating a second check after the frontend already called `check_for_updates()`. Between these two checks, a newer version could be published — user approved version X but version Y gets installed.

**Impact:** User consent mismatch. Not a security issue (signature verification still holds), but a correctness/trust issue.

**Mitigation options:**
1. Accept as-is (race window is seconds, very rare)
2. Cache last check result in `AppContextState` and reuse in install
3. Document as known limitation

### 13.2 Concurrency guard needed for install (RAII pattern)

**Problem:** No `Mutex`, `AtomicBool`, or other serialization prevents concurrent `install_update` calls. If user double-clicks or background check + user trigger fire simultaneously, two installer processes can race on the same temp files.

**Impact:** Corrupted download, installer conflicts, half-updated state.

**Fix:** Use RAII guard pattern — `try_acquire()` returns `Option<InstallGuardGuard>` whose `Drop` impl releases the lock. This ensures the guard is released even if the future panics:

```rust
// In AppContextState
pub install_guard: infrastructure::updater::update_checker::InstallGuard,

// In execute_install_update:
let _guard = install_guard
    .try_acquire()
    .ok_or_else(|| "An update is already being installed".to_string())?;
// _guard auto-releases on drop (including panic)
download_and_install_update(app).await
```

**WRONG** — manual release (not panic-safe):
```rust
// ❌ If download_and_install_update panics, release() is never called
if !install_guard.try_acquire() { return Err(...); }
let result = download_and_install_update(app).await;
install_guard.release();  // unreachable on panic
result
```

### 13.3 Event deduplication for periodic checks

**Problem:** Startup check emits `"update-available"`, then periodic check (24h later) emits again for the same version. Frontend may show duplicate notifications.

**Impact:** User sees repeated "update available" popups for the same version.

**Fix:** Track last emitted version in state, only emit when version changes:
```rust
// In AppContextState
last_emitted_update_version: Arc<Mutex<Option<String>>>,

// Before emitting:
let mut last_version = last_emitted_update_version.lock().unwrap();
if last_version.as_ref() != result.version.as_ref() {
    let _ = update_handle.emit("update-available", &result);
    *last_version = result.version.clone();
}
```

### 13.4 Emit DTO, not infrastructure type

**Problem:** Background task emits `UpdateCheckResult` (infrastructure type) via `update_handle.emit()`. Frontend contract is implicitly coupled to infrastructure layer.

**Impact:** If `UpdateCheckResult` gains/renames a field, frontend contract silently breaks.

**Fix:** Convert to DTO before emitting:
```rust
let dto = UpdateCheckResponse {
    update_available: result.update_available,
    version: result.version,
    release_notes: result.release_notes,
};
let _ = update_handle.emit("update-available", &dto);
```

### 13.5 Add signing key patterns to .gitignore

**Problem:** `docs/code-signing.md` instructs developers to add signing key to `.gitignore`, but the project's `.gitignore` doesn't include patterns for `*.key`, `.tauri/`, or signing-related files.

**Impact:** If developer generates key inside project tree (e.g., `cargo tauri signer generate -w .tauri/sapo-printer.key`), private key can be committed on next `git add .`.

**Fix:** Add to `.gitignore`:
```gitignore
# Tauri signing keys
*.key
*.key.pub
.tauri/
```

### 13.6 Known limitations (deferred, acceptable for now)

These were identified but deferred as out-of-scope or plugin limitations:

- **No network timeout:** tauri-plugin-updater doesn't set timeout on reqwest client. If endpoint unreachable, check hangs indefinitely. Consider wrapping in `tokio::time::timeout()`.
- **No download progress event:** `on_chunk` closure only logs via tracing, doesn't emit Tauri event. Frontend can't show progress bar. Consider emitting `"update-download-progress"` event.
- **No cancellation mechanism:** Spawned task runs infinite loop with no `CancellationToken`. Consider adding graceful shutdown.
- **No panic recovery:** If spawned task panics, periodic checks stop silently. Consider `catch_unwind` or supervisor pattern.
- **install_update returns no version info:** Command returns `Result<(), String>`, frontend doesn't know which version is installing. Consider returning `Result<String, String>`.

## 14. Frontend restart after update — `window.location.reload()` does NOT restart the native process

After `install_update` completes and the `"update-ready-to-apply"` event fires, the frontend must restart the app so the new binary takes effect.

**WRONG** — webview reload only:
```typescript
// ❌ This only reloads the webview page content
// The native process keeps running with the OLD binary
const handleRestart = () => {
  window.location.reload();
};
```

**Why it fails:** In Tauri, `window.location.reload()` reloads the webview's HTML/JS/CSS but does NOT terminate the native Rust process. The update binary has been written to disk, but the running process is still the old one. The new version won't take effect until the process exits and the user launches the app again.

**RIGHT** — backend command that exits the process:

```rust
// In commands/update.rs:
pub fn execute_restart_app() -> Result<(), String> {
    std::process::exit(0);
}

// In main.rs:
#[tauri::command]
fn restart_app() -> Result<(), String> {
    sapo_printer::interface::tauri::commands::update::execute_restart_app()
}
```

```typescript
// In services/update-service.ts:
export async function restartApp(): Promise<void> {
  return invoke('restart_app');
}

// In UpdatePopup.tsx:
const handleRestart = async () => {
  try {
    await restartApp();
  } catch {
    // Fallback if IPC fails
    window.location.reload();
  }
};
```

**Alternative:** Use `@tauri-apps/plugin-process` (`process.restart()`) — but this adds an npm dependency. `std::process::exit(0)` is simpler and achieves the same result (OS launches new binary on next app start).

**Note:** `app.restart()` from Tauri's `Manager` trait would also work but kills the app immediately from the backend side. Using `std::process::exit(0)` gives the frontend control over when to trigger the exit (e.g., after showing a "Restarting..." message).
