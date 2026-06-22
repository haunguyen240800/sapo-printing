# Deferred Work

## Deferred from: code review of story 1.2 (2026-06-22)

- `now_unix()` trong `events.rs` gọi trực tiếp `SystemTime::now()` — event constructors không thể mock timestamp trong tests. Testability concern, sẽ xử lý khi cần inject clock dependency.

## Deferred from: code review of 1-3-create-printer-domain-model (2026-06-22)

- `FromStr` impl trên `PrinterId` không được spec yêu cầu, không có test cover — `src-tauri/src/domain/printer/value_objects.rs:28`. Nên thêm test hoặc remove nếu không cần thiết.


## Deferred from: code review of 1-3-create-printer-domain-model Round 2 (2026-06-22)

- `set_error()` emit `PrinterDisconnected` thay vì `PrinterErrored` — per-spec AC-2 chỉ định 2 event types; consumer dùng `status()` để phân biệt. Xem xét thêm `PrinterErrored` khi cần consumer phân biệt error vs clean disconnect.
- `PrinterName::new` không validate empty string — không phải domain rule per spec 1.3; upstream validate nếu cần.
- `PrinterRepository::save` nhận `&Printer` — intentional pattern (Story 1.2 precedent); event drain ở application layer.
- `PrinterRepository` thiếu `find_by_id` — không trong spec 1.3; add trong Story 2.x khi Infrastructure cần.

## Deferred from: code review of 1-4-create-document-domain-model-appcontext-di-container (2026-06-22)

- `DocumentLocation::new` accepts any string without URL validation — intentional per spec; validation is Document::new()'s responsibility.
- HTTP URLs accepted in Document::new() — MITM/plaintext risk; security hardening is Epic 2+ concern.
- `AppContext::new` panics via `todo!()` — intentional per spec dev notes; real wiring in Epic 2 (Story 2.4).
- `AppContext` fields all `pub` — encapsulation refactor post-Epic-2 when constructor is implemented.
- `EventBus::publish` uses untyped `&str` payload — intentional "simple string-based interface for now" per spec.
- `DocumentId` has no `From<Uuid>`/inner accessor for persistence reconstruction — Epic 2+ concern.
- `DocumentType` missing `#[non_exhaustive]` — valid guard against silent breaking changes; out of scope for story 1.4.
- `platform_engine_name` returns `"cups"` on all non-Windows including non-CUPS targets — project scope is Tauri desktop (Windows/macOS/Linux) only.
- Scheme-only URL (`https://`) passes validation guard — no host presence check in `Document::new()` [`aggregate.rs:18`].
- Uppercase scheme (`HTTP://`, `HTTPS://`) rejected incorrectly — scheme check is case-sensitive, RFC 3986 allows case-insensitive [`aggregate.rs:18`].
- URL with leading whitespace rejected — no trim before validation in `Document::new()` [`aggregate.rs:18`].
