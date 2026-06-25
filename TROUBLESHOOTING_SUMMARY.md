# Troubleshooting Summary: Print Flow Not Working

## Problem
Jobs are created but not printed. Log shows:
```
✅ CreatePrintJobUseCase: starting
✅ Saving job to repository
❌ (No further logs - flow stops here)
```

## Root Cause Analysis

### Expected Flow
```
1. CreatePrintJobUseCase
2. job_repo.save(job)           ✅ Working
3. event_store.save_all(events) ❌ BLOCKING HERE
4. event_bus.publish()
5. PushToQueueHandler
6. QueueManager.push()
7. QueueWorker picks up job
8. Download → Render → Print
```

### Blocking Point
Flow stops at **step 3**: `event_store.save_all()`

**Why?** The EventStore needs an HMAC signing key from Windows Credential Manager, which uses blocking Win32 API calls:

```rust
// In event_store.rs line 172
let signing_key = self.get_or_create_signing_key()?;  // BLOCKS HERE

// Calls WindowsCredentialManager
secret_manager.retrieve("hmac_signing_key")  // CredReadW() - blocking
secret_manager.store("hmac_signing_key", key) // CredWriteW() - blocking
```

### Windows Credential Manager Issue
The Win32 API calls (`CredReadW` and `CredWriteW`) are synchronous and can be slow:
- First-time access may require Windows authentication
- Network credential sync can cause delays
- DPAPI encryption/decryption overhead
- Antivirus scanning of credential operations

## Enhanced Logging Added

### Files Modified
1. **event_store.rs** - Detailed logging for `save_all()` and `get_or_create_signing_key()`
2. **windows_credential_manager.rs** - Log all Win32 API calls
3. **push_to_queue_handler.rs** - Log event reception
4. **tauri_event_bus.rs** - Log event publishing and handler counts
5. **sqlite_queue_manager.rs** - Log push/pop operations
6. **queue_worker.rs** - Log polling activity

### Expected Debug Output
When you run the app now, you should see:

```log
# Step 1: Job creation
CreatePrintJobUseCase: starting

# Step 2: Save to repository
Saving job to repository

# Step 3: Save events (THIS IS WHERE IT BLOCKS)
save_all() STARTING
save_all() - calling get_or_create_signing_key()
get_or_create_signing_key() - checking cache
get_or_create_signing_key() - calling secret_manager.retrieve()
WindowsCredentialManager::retrieve() - STARTING
WindowsCredentialManager::retrieve() - calling CredReadW()
# ⏱️ LONG DELAY HERE - Windows API blocking
WindowsCredentialManager::retrieve() - CredReadW() SUCCESS (or key not found)
# If not found, will generate and store:
get_or_create_signing_key() - generating new key
WindowsCredentialManager::store() - STARTING
WindowsCredentialManager::store() - calling CredWriteW()
# ⏱️ ANOTHER DELAY - Windows API blocking
WindowsCredentialManager::store() - CredWriteW() SUCCESS
get_or_create_signing_key() - caching new key

# Step 4: Continue with event publishing
save_all() - signing key retrieved successfully
Publishing events
EventBus: publishing event (PrintJobCreated)
EventBus: found handlers (handler_count=1)

# Step 5: Handler processes event
PushToQueueHandler: received event
SqliteQueueManager: push() called
SqliteQueueManager: job status updated to Queued

# Step 6: Worker picks up job
QueueWorker: picked up job for processing
# ... download/render/print logs
```

## How to Debug

### Step 1: Run with full logging
```bash
$env:RUST_LOG="info"
cd src-tauri
cargo run
```

### Step 2: Create a test print job from UI

### Step 3: Check log output
Look for where the log **stops**:

**Scenario A: Stops after "calling CredReadW()"**
- Windows Credential Manager is blocking
- Possible causes:
  - First-time access requires Windows authentication
  - Antivirus scanning credentials
  - Network sync delay (if domain-joined machine)

**Scenario B: Stops after "CredReadW() SUCCESS" but before "signing key retrieved"**
- Issue with key caching or error handling

**Scenario C: Stops after "signing key retrieved"**
- Issue with database transaction or event insertion

### Step 4: Check Windows Credential Manager
```
1. Open Control Panel → Credential Manager
2. Go to "Windows Credentials" section
3. Look for "sapo-printer:hmac_signing_key"
4. If it exists, try deleting it and restart the app
```

## Quick Fix Attempts

### Fix 1: Pre-initialize the signing key at startup
In `main.rs`, after creating EventStore, call:
```rust
// Force signing key initialization at startup (before any jobs)
event_store.get_or_create_signing_key()
    .expect("Failed to initialize signing key");
tracing::info!("Signing key initialized successfully");
```

This moves the blocking operation to startup time instead of first job creation.

### Fix 2: Use in-memory secret storage (dev/testing only)
Create a mock `InMemorySecretManager` that doesn't use Windows Credential Manager:
```rust
struct InMemorySecretManager {
    secrets: Arc<Mutex<HashMap<String, String>>>,
}
```

Replace WindowsCredentialManager with this for testing.

### Fix 3: Increase spawn_blocking timeout
The blocking task might be timing out. Check Tokio runtime configuration.

## Long-term Solutions

### Option 1: Async Secret Manager
Wrap Windows Credential Manager calls in async operations:
```rust
async fn retrieve_async(&self, key: &str) -> Result<Option<String>, Error> {
    tokio::task::spawn_blocking(move || {
        // Call CredReadW here
    }).await?
}
```

### Option 2: Cache signing key at startup
Store the signing key in memory at application startup and never call Windows Credential Manager again during runtime.

### Option 3: Use SQLite for secret storage
Store encrypted secrets in the database instead of Windows Credential Manager:
- Faster (no OS API calls)
- More reliable (no authentication prompts)
- Still secure (encrypt with machine-specific key)

## Next Steps

1. **Run the app with logging** and share the complete log output
2. **Identify exactly where it blocks** (which log line is the last one)
3. **Check Windows Credential Manager** for the `sapo-printer:hmac_signing_key` entry
4. **Try Quick Fix #1** - pre-initialize the signing key at startup

Once we know the exact blocking point, we can implement the appropriate fix.

## Files to Review
- `src-tauri/src/infrastructure/database/event_store.rs` (line 75-118, 162-231)
- `src-tauri/src/infrastructure/secrets/windows_credential_manager.rs`
- `src-tauri/src/application/use_cases/create_print_job.rs` (line 102-131)
- `src-tauri/src/main.rs` (EventStore initialization)
