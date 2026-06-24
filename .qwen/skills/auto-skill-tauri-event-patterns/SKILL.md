---
name: tauri-event-patterns
description: Tauri 2.0 event emission and listening patterns — AppHandle availability, thread safety, and frontend subscription API
source: auto-skill
extracted_at: '2026-06-24T09:14:43.708Z'
---

# Tauri Event Emission and Listening Patterns

Learned from implementing story 3-9 (real-time status updates). These patterns are specific to Tauri 2.0 and this project's architecture.

## 1. Tauri 2.0 Event Emission API — use `emit()`, not `emit_all()`

Tauri 2.0 changed the event emission API. Use `app_handle.emit()` for broadcasting events to all windows.

**WRONG** — Tauri 1.x API (deprecated in 2.0):

```rust
use tauri::Manager;
app.emit_all("event_name", payload)?;  // ❌ Not available in Tauri 2.0
```

**RIGHT** — Tauri 2.0 API:

```rust
use tauri::Emitter;  // Import the Emitter trait

app_handle.emit("event_name", payload)?;  // ✅ Correct for Tauri 2.0
```

**Key differences:**
- Import `tauri::Emitter` trait (not `tauri::Manager`)
- Method is `emit()`, not `emit_all()`
- `AppHandle` implements `Emitter` trait

## 2. AppHandle is only available in `.setup()` callback

`AppHandle` is created by Tauri during app initialization and is only accessible within the `.setup()` closure. You cannot access it in `main()` before `.setup()` runs.

**WRONG** — trying to access AppHandle in main():

```rust
fn main() {
    let app_handle = ???;  // ❌ Not available yet
    
    let event_bus = TauriEventBus::new(app_handle);  // ❌ Cannot create here
    
    tauri::Builder::default()
        .manage(AppContextState { event_bus, ... })
        .run(...)
}
```

**RIGHT** — create dependencies inside `.setup()`:

```rust
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle().clone();  // ✅ Available here
            
            // Create dependencies that need AppHandle
            let event_bus = Arc::new(TauriEventBus::new(app_handle.clone()));
            
            // Create other dependencies
            let queue_worker = Arc::new(QueueWorker::new(..., event_bus, ...));
            queue_worker.start()?;
            
            // Manage state with AppHandle
            app.manage(AppContextState {
                event_bus,
                queue_worker,
                app_handle,  // Store for later use if needed
                ...
            });
            
            Ok(())
        })
        .run(...)
}
```

**Pattern:** Move all dependency initialization (DB, repositories, workers) into `.setup()` if they need `AppHandle`.

## 3. Frontend Event Listening — use `@tauri-apps/api/event`

Tauri 2.0 split the frontend API into separate modules. Event listening uses a different import path than commands.

**WRONG** — Tauri 1.x import path:

```typescript
import { listen } from '@tauri-apps/api/tauri';  // ❌ Deprecated in 2.0
```

**RIGHT** — Tauri 2.0 import paths:

```typescript
// For event listening
import { listen, UnlistenFn } from '@tauri-apps/api/event';  // ✅

// For invoking commands
import { invoke } from '@tauri-apps/api/core';  // ✅
```

**Complete event subscription pattern:**

```typescript
import { listen, UnlistenFn } from '@tauri-apps/api/event';

interface EventPayload {
  job_id: string;
  status: string;
  progress: number;
}

// In React component
useEffect(() => {
  let unlisten: UnlistenFn | null = null;

  const subscribe = async () => {
    unlisten = await listen<EventPayload>('event_name', (event) => {
      console.log('Received:', event.payload);
      // Update state with event.payload
    });
  };

  subscribe();

  // Cleanup on unmount
  return () => {
    if (unlisten) unlisten();
  };
}, []);
```

**Critical:** Always call the `unlisten` function on component unmount to prevent memory leaks.

## 4. Thread Safety — `AppHandle.emit()` is thread-safe

`AppHandle` implements `Clone + Send + Sync`, and `emit()` can be called from any thread, including background worker threads.

**Safe pattern for background workers:**

```rust
pub struct TauriEventBus {
    app_handle: AppHandle,  // AppHandle is Clone + Send + Sync
}

impl EventBus for TauriEventBus {
    fn publish(&self, event_type: &str, payload: &str) -> Result<(), EventBusError> {
        // Safe to call from background thread
        self.app_handle.emit("event_name", json_payload)
            .map_err(|e| EventBusError::PublishFailed { reason: e.to_string() })?;
        Ok(())
    }
}
```

**No mutex needed:** `AppHandle` is designed for concurrent access. You can clone it and share across threads without synchronization.

## 5. Event Payload Structure — flat JSON for performance

Tauri events serialize payloads to JSON. Use flat structures (not nested) for better performance and simpler frontend consumption.

**GOOD** — flat payload:

```rust
let payload = serde_json::json!({
    "job_id": "abc-123",
    "status": "PRINTING",
    "progress": 80,
    "error_message": null
});

app_handle.emit("job_status_changed", payload)?;
```

**Frontend consumption:**

```typescript
interface JobStatusPayload {
  job_id: string;
  status: string;
  progress: number;
  error_message?: string;
}

listen<JobStatusPayload>('job_status_changed', (event) => {
  const { job_id, status, progress } = event.payload;  // ✅ Direct access
});
```

**Avoid deeply nested structures** — they add serialization overhead and complicate TypeScript types.

## 6. Event Naming Convention — snake_case

Use `snake_case` for event names to match Rust conventions and maintain consistency.

**Examples:**
- `job_status_changed` ✅
- `download_progress` ✅
- `print_progress` ✅
- `printer_status_changed` ✅

**Frontend must match exactly:**

```typescript
listen('job_status_changed', handler);  // Must match backend emit name
```

## 7. Implementing EventBus Trait with TauriEventBus

When replacing `InMemoryEventBus` with `TauriEventBus`, maintain the same trait interface.

**Pattern:**

```rust
// Shared trait (in shared/event_bus.rs)
pub trait EventBus: Send + Sync {
    fn publish(&self, event_type: &str, payload: &str) -> Result<(), EventBusError>;
}

// Production implementation (in infrastructure/eventbus/tauri_event_bus.rs)
pub struct TauriEventBus {
    app_handle: AppHandle,
}

impl TauriEventBus {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle }
    }
}

impl EventBus for TauriEventBus {
    fn publish(&self, event_type: &str, payload: &str) -> Result<(), EventBusError> {
        // Map domain event → Tauri event
        let tauri_event = match event_type {
            "PrintJobCreated" | "PrintJobQueued" | ... => "job_status_changed",
            _ => return Ok(()),  // Ignore unknown events
        };
        
        // Parse domain payload → extract UI-relevant fields
        let domain_payload: serde_json::Value = serde_json::from_str(payload)
            .map_err(|e| EventBusError::PublishFailed { reason: e.to_string() })?;
        
        // Build flat UI payload
        let ui_payload = serde_json::json!({
            "job_id": domain_payload["job_id"],
            "status": event_type_to_status(event_type),
            "progress": status_to_progress(event_type),
        });
        
        // Emit to frontend
        self.app_handle.emit(tauri_event, ui_payload)
            .map_err(|e| EventBusError::PublishFailed { reason: e.to_string() })?;
        
        Ok(())
    }
}

// Keep InMemoryEventBus for tests
pub struct InMemoryEventBus;

impl EventBus for InMemoryEventBus {
    fn publish(&self, _event_type: &str, _payload: &str) -> Result<(), EventBusError> {
        Ok(())  // No-op
    }
}
```

**Key:** Both implementations satisfy the same trait, so production code uses `TauriEventBus` while tests use `InMemoryEventBus`.

## 8. Debouncing Rapid Event Updates

When events fire rapidly (e.g., progress updates), debounce on the frontend to prevent excessive re-renders.

**Per-job debounce pattern (200ms window):**

```typescript
const lastUpdateRef = useRef<Map<string, number>>(new Map());
const pendingUpdateRef = useRef<Map<string, NodeJS.Timeout>>(new Map());

const handleEvent = useCallback((payload: EventPayload) => {
  const now = Date.now();
  const lastUpdate = lastUpdateRef.current.get(payload.job_id) || 0;
  const elapsed = now - lastUpdate;

  if (elapsed >= 200) {
    // Apply immediately
    lastUpdateRef.current.set(payload.job_id, now);
    applyUpdate(payload);
  } else {
    // Debounce: schedule after remaining time
    const existing = pendingUpdateRef.current.get(payload.job_id);
    if (existing) clearTimeout(existing);

    const timeout = setTimeout(() => {
      lastUpdateRef.current.set(payload.job_id, Date.now());
      pendingUpdateRef.current.delete(payload.job_id);
      applyUpdate(payload);
    }, 200 - elapsed);

    pendingUpdateRef.current.set(payload.job_id, timeout);
  }
}, []);

// Cleanup on unmount
useEffect(() => {
  return () => {
    pendingUpdateRef.current.forEach(timeout => clearTimeout(timeout));
  };
}, []);
```

**Why:** Prevents UI thrashing when multiple events arrive in quick succession for the same job.
