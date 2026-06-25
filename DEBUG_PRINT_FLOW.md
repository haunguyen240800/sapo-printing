# Debug Print Flow - Không thấy action in

## Vấn đề
Log hiện tại chỉ thấy:
- `CreatePrintJobUseCase: starting` ✅
- `Saving job to repository` ✅
- `Saving events to event store` ✅
- `Publishing events` ✅

Nhưng KHÔNG thấy:
- ❌ Handler nhận event `PrintJobCreated`
- ❌ Job được push vào queue
- ❌ Worker xử lý job
- ❌ Download/Render/Print actions

## Đã thêm enhanced logging

Đã thêm log chi tiết vào các component sau:

### 1. PushToQueueHandler
```rust
// Log khi nhận event
tracing::info!("PushToQueueHandler: received event")

// Log khi ignore event
tracing::debug!("PushToQueueHandler: ignoring non-PrintJobCreated event")

// Log khi push thành công
tracing::info!("Job pushed to queue successfully")
```

### 2. TauriEventBus
```rust
// Log khi publish event
tracing::info!("EventBus: publishing event")

// Log số lượng handlers đăng ký
tracing::info!("EventBus: found handlers for event", handler_count = X)
```

### 3. SqliteQueueManager
```rust
// Log khi push
tracing::info!("SqliteQueueManager: push() called")
tracing::info!("SqliteQueueManager: job status updated to Queued")

// Log khi pop
tracing::debug!("SqliteQueueManager: pop() called")
tracing::info!("SqliteQueueManager: popped job, updating status to Pending")
tracing::trace!("SqliteQueueManager: no jobs available in queue")
```

### 4. QueueWorker
```rust
// Log mỗi 10 polls
tracing::debug!("QueueWorker: still running, polling queue", poll_count = X)

// Log khi pick job
tracing::info!("QueueWorker: picked up job for processing")
```

## Các bước để debug

### Bước 1: Chạy app với logging level cao
```bash
# Set RUST_LOG environment variable
$env:RUST_LOG="debug"

# Hoặc trong PowerShell
$env:RUST_LOG="sapo_printer=debug,info"

# Chạy app
cd src-tauri
cargo run
```

### Bước 2: Tạo print job từ UI
Vào UI và tạo 1 print job với URL test.

### Bước 3: Quan sát log theo thứ tự expected

**Expected log flow:**
```
1. CreatePrintJobUseCase: starting
2. Saving job to repository
3. Saving events to event store
4. Publishing events
5. EventBus: publishing event (event_type=PrintJobCreated)
6. EventBus: found handlers for event (handler_count=1)
7. PushToQueueHandler: received event
8. SqliteQueueManager: push() called
9. SqliteQueueManager: job status updated to Queued
10. Job pushed to queue successfully
11. QueueWorker: still running, polling queue
12. SqliteQueueManager: pop() called
13. SqliteQueueManager: popped job, updating status to Pending
14. QueueWorker: picked up job for processing
15. [Download/Render/Print logs...]
```

### Bước 4: Xác định điểm bị gián đoạn

Nếu log dừng lại ở:
- **Sau step 4** → EventBus không publish hoặc không có handlers đăng ký
- **Sau step 6** → Handler không được gọi (thread spawn issue)
- **Sau step 9** → Worker không poll hoặc không tìm thấy job
- **Sau step 13** → Worker xử lý job bị crash

## Kiểm tra Database

### Kiểm tra job status trong DB
```bash
# Tìm database file
ls ~/.sapo-printer/config.db  # Linux/Mac
ls $env:USERPROFILE/.sapo-printer/config.db  # Windows

# Query database
sqlite3 ~/.sapo-printer/config.db
# Hoặc Windows
sqlite3 "$env:USERPROFILE\.sapo-printer\config.db"

# Check jobs
SELECT id, status, retry_count, scheduled_at, created_at
FROM print_jobs
ORDER BY created_at DESC
LIMIT 5;

# Check queue depth
SELECT COUNT(*) FROM print_jobs WHERE status = 'Queued';
```

### Expected states
- **Pending** → Job created, waiting for push to queue
- **Queued** → Job in queue, waiting for worker to pick up
- **Downloaded** → Worker downloaded file
- **Printing** → Worker sending to printer
- **Completed** → Done

## Các nguyên nhân có thể

### 1. Handler không được đăng ký đúng
**Check:** Log sẽ có `handler_count=0` thay vì `handler_count=1`

**Fix:** Verify trong `main.rs`:
```rust
event_bus.subscribe("PrintJobCreated", push_handler);
tracing::info!("PushToQueueHandler registered for PrintJobCreated events");
```

### 2. Worker không được start
**Check:** Không thấy log `QueueWorker: processing loop started`

**Fix:** Verify trong `main.rs`:
```rust
worker.start().expect("Failed to start queue worker");
```

### 3. Database locking issue
**Check:** Log có error về `SQLITE_BUSY` hoặc `database is locked`

**Fix:** Database đã config WAL mode và busy_timeout, nhưng có thể cần kiểm tra:
```rust
PRAGMA journal_mode=WAL;
PRAGMA synchronous=NORMAL;
busy_timeout(5 seconds)
```

### 4. Thread spawn trong handler bị block
**Check:** Log thấy event published nhưng handler không chạy

**Fix:** Handler spawn background thread:
```rust
std::thread::spawn(move || {
    handler_clone.handle(&event_type_owned, &payload_owned);
});
```

## Next Steps

Sau khi chạy với enhanced logging:

1. Copy **toàn bộ log output** từ console
2. Xác định **step cuối cùng** xuất hiện trong log
3. Kiểm tra **database state** với SQL queries
4. Report lại với thông tin:
   - Log đầy đủ
   - Database query results
   - Điểm gián đoạn cụ thể

## Build & Run Commands

```bash
# Build
cd src-tauri
cargo build

# Run with logging
$env:RUST_LOG="debug"
cargo run

# Or run built binary
cd target/debug
./sapo-printer.exe
```
