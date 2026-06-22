# Deferred Work

## Deferred from: code review of story 1.2 (2026-06-22)

- `now_unix()` trong `events.rs` gọi trực tiếp `SystemTime::now()` — event constructors không thể mock timestamp trong tests. Testability concern, sẽ xử lý khi cần inject clock dependency.
