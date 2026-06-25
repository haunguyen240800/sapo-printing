# Database Deadlock Fix - Root Cause & Solution

## Problem Identified

Print jobs were created but never printed. Log showed:
```
✅ CreatePrintJobUseCase: starting
✅ Saving job to repository
✅ SqlitePrintJobRepository::save() - STARTING
❌ (BLOCKED FOREVER - never gets database lock)
```

## Root Cause: Database Lock Contention

### The Deadlock Scenario

1. **GetMetricsUseCase** runs every 5 seconds (from UI polling)
2. **MetricsCollector** acquires database lock and holds it for the entire collection:
   ```rust
   let conn = self.conn.lock()?;  // LOCK ACQUIRED
   collect_job_metrics(&conn);      // Multiple SELECT queries
   collect_queue_metrics(&conn);
   collect_printer_metrics(&conn);
   collect_performance_metrics(&conn);
   // Lock released when conn drops at end of function
   ```

3. If **CreatePrintJobUseCase** is called while metrics collection is running:
   ```rust
   let conn = self.conn.lock()?;  // WAITS FOR LOCK
   // BLOCKS FOREVER if metrics takes >30s or never completes
   ```

### Why It Blocks Forever

- **Single shared database connection** (`Arc<Mutex<Connection>>`)
- **Metrics queries can be slow** on large tables (GROUP BY, aggregations)
- **busy_timeout was only 5 seconds** (too short)
- **No explicit lock release** in metrics collector

### Evidence from Logs

```log
14:32:18.242096  CreatePrintJobUseCase: Saving job to repository
14:32:18.242210  SqlitePrintJobRepository::save() - STARTING
(NO FURTHER LOGS - blocked on lock acquisition)

14:32:19.997204  GetMetricsUseCase: starting  ← Running concurrently!
14:32:24.999945  GetMetricsUseCase: starting  ← Still running!
14:32:29.997224  GetMetricsUseCase: starting  ← Still running!
```

GetMetricsUseCase was holding the lock for 5+ seconds while create_print_job was blocked.

## Solution Implemented

### Fix 1: Increase busy_timeout (30 seconds)
**File:** `src-tauri/src/infrastructure/database/connection.rs`

```rust
// Changed from 5 seconds to 30 seconds
conn.busy_timeout(std::time::Duration::from_secs(30))
```

This gives SQLite more time to wait for lock release before giving up.

### Fix 2: Explicit Lock Release in MetricsCollector
**File:** `src-tauri/src/infrastructure/metrics/collector.rs`

```rust
pub fn collect_metrics(&self) -> Result<MetricsSnapshot, MetricsError> {
    let conn = self.conn.lock()?;

    let job_metrics = self.collect_job_metrics(&conn)?;
    let queue_metrics = self.collect_queue_metrics(&conn)?;
    let printer_metrics = self.collect_printer_metrics(&conn)?;
    let performance_metrics = self.collect_performance_metrics(&conn)?;

    // Explicitly drop lock ASAP
    drop(conn);  // ← NEW

    // Do timestamp calculation without holding lock
    let collected_at = SystemTime::now()...

    Ok(MetricsSnapshot {...})
}
```

This releases the lock as soon as data collection is done, rather than holding it during timestamp calculation.

### Fix 3: Lock Wait Time Logging
**File:** `src-tauri/src/infrastructure/database/print_job_repository.rs`

```rust
let lock_start = std::time::Instant::now();
let conn = self.conn.lock()?;
let lock_duration = lock_start.elapsed();

tracing::info!("got database lock", lock_wait_ms = lock_duration.as_millis());

if lock_duration.as_secs() > 5 {
    tracing::warn!("SLOW LOCK (waited >5s)", lock_wait_secs = ...);
}
```

This logs how long each operation waits for the lock, helping diagnose future contention issues.

### Fix 4: Enhanced Logging
Added detailed logging to:
- MetricsCollector: lock acquisition/release
- PrintJobRepository: lock wait time
- All component operations

## Expected Behavior After Fix

### Successful Flow
```log
14:32:18  CreatePrintJobUseCase: starting
14:32:18  Saving job to repository
14:32:18  SqlitePrintJobRepository::save() - STARTING
14:32:18  MetricsCollector: acquiring database lock
14:32:18  MetricsCollector: lock acquired
14:32:18  MetricsCollector: lock released  ← Quick release
14:32:18  SqlitePrintJobRepository::save() - got database lock (waited 50ms)
14:32:18  SqlitePrintJobRepository::save() - INSERT SUCCESS
14:32:18  Saving events to event store
14:32:18  Publishing events
14:32:18  PushToQueueHandler: received event
14:32:18  Job pushed to queue
14:32:18  QueueWorker: picked up job
14:32:20  [Download/Render/Print logs...]
```

### If Still Slow
```log
14:32:18  SqlitePrintJobRepository::save() - STARTING
14:32:23  SqlitePrintJobRepository::save() - got database lock (waited 5200ms)
14:32:23  SLOW LOCK (waited >5s)  ← Warning logged
14:32:23  SqlitePrintJobRepository::save() - INSERT SUCCESS
```

## Testing

Run the app and create a print job:

```bash
cargo run --manifest-path=src-tauri/Cargo.toml
```

**Expected:**
- Job should be saved within 1-2 seconds
- No blocking or hangs
- Logs should show lock wait time <100ms under normal conditions
- If metrics is running: lock wait time may be 100-500ms but should succeed

**If still blocking:**
- Check logs for "SLOW LOCK" warnings
- Look for lock_wait_ms values >5000
- May need to implement connection pooling (separate connections for metrics and jobs)

## Long-term Improvements

### Option A: Separate Connection Pool
Create separate database connections:
- **Read connection** for metrics (read-only queries)
- **Write connection** for job creation (INSERT/UPDATE)

SQLite WAL mode supports concurrent reads and writes.

### Option B: Cache Metrics in Memory
Instead of querying database every 5 seconds:
- Update metrics in-memory when jobs change
- Persist to database periodically (every minute)
- Serve metrics from memory cache

### Option C: Reduce Metrics Polling Frequency
Change UI to poll every 10-15 seconds instead of 5 seconds.

### Option D: Async Database Access
Use `tokio-rusqlite` or `sqlx` for async database operations with proper connection pooling.

## Files Modified

1. `src-tauri/src/infrastructure/database/connection.rs` - Increased busy_timeout to 30s
2. `src-tauri/src/infrastructure/metrics/collector.rs` - Explicit lock release + logging
3. `src-tauri/src/infrastructure/database/print_job_repository.rs` - Lock wait time logging
4. `src-tauri/src/main.rs` - Enhanced error logging
5. `src-tauri/src/application/use_cases/create_print_job.rs` - Step-by-step logging

## Verification

After applying fixes, verify:

✅ Print jobs are created without blocking
✅ Logs show "INSERT SUCCESS" within 1-2 seconds
✅ Events are published and jobs are queued
✅ QueueWorker processes jobs successfully
✅ No "SLOW LOCK" warnings under normal load

If issues persist, check for:
- Database corruption (delete and recreate `config.db`)
- Metrics queries taking >10s (optimize with indexes)
- Other components holding locks (check all `.lock()` calls)
