---
title: 'Đổi mã lỗi pairing khi desktop chưa sẵn sàng xác nhận'
type: 'refactor'
created: '2026-08-24'
status: 'done'
baseline_commit: '91b352ecdaae8353db1c5824211553f39d610063'
context:
  - '{project-root}/CLAUDE.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** API `POST /api/v1/pair` đang trả mã `no_ui_subscriber`, làm lộ chi tiết triển khai channel nội bộ và không diễn đạt rõ hành động FE/người dùng cần hiểu.

**Approach:** Đổi duy nhất API error code công khai thành `pairing_confirmation_unavailable`, diễn đạt rằng ứng dụng desktop hiện không thể tiếp nhận yêu cầu xác nhận pairing; giữ nguyên HTTP 503 và biến thể lỗi nội bộ `PairError::NoUiSubscriber`.

## Boundaries & Constraints

**Always:** Giữ response JSON `{ "code", "message" }`; giữ HTTP 503; tên code phải ổn định, machine-readable và mô tả ngữ nghĩa bên ngoài thay vì cơ chế `mpsc` bên trong.

**Ask First:** Bất kỳ thay đổi nào đối với tên enum nội bộ, quy trình pairing, timeout, CORS hoặc cơ chế cấp token.

**Never:** Duy trì alias cho mã cũ; thay đổi các mã `user_denied`, `pair_timeout`; mở rộng sang chuẩn hóa toàn bộ error response API.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Desktop confirmation channel unavailable | `request_pair` trả `PairError::NoUiSubscriber` | HTTP 503 với JSON code `pairing_confirmation_unavailable` | Message không nhắc tới subscriber/channel nội bộ |
| Các lỗi pairing khác | User từ chối, timeout hoặc backend error | Giữ nguyên status, code và message hiện tại | Không regression API contract ngoài mã được đổi |

</frozen-after-approval>

## Code Map

- `src-tauri/src/interface/http_server/handlers.rs` — chuyển `PairError::NoUiSubscriber` thành HTTP error response công khai.
- `src-tauri/src/application/ports/api_token_port.rs` — định nghĩa lỗi nội bộ; chỉ dùng làm ngữ cảnh, không sửa.
- `src-tauri/src/infrastructure/persistence/api_token_repository.rs` — nơi phát sinh lỗi khi UI sink thiếu/đóng; chỉ dùng làm ngữ cảnh, không sửa.

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/src/interface/http_server/handlers.rs` — đổi public error code và message của nhánh `PairError::NoUiSubscriber`.
- [x] `src-tauri/src/interface/http_server/handlers.rs` — bổ sung kiểm thử khóa HTTP status, code và message của mapping để tránh regression.

**Acceptance Criteria:**
- Given desktop confirmation channel chưa được đăng ký hoặc đã đóng, when web FE gọi `POST /api/v1/pair`, then API trả HTTP 503 với JSON code `pairing_confirmation_unavailable`.
- Given cùng lỗi trên, when FE đọc message, then message mô tả không thể yêu cầu xác nhận pairing và không chứa thuật ngữ `subscriber`.
- Given các nhánh pairing còn lại, when mapping lỗi, then contract hiện tại không thay đổi.

## Spec Change Log

## Verification

**Commands:**
- `cargo test interface::http_server::handlers --lib` — expected: kiểm thử mapping pairing pass.
- `cargo fmt --check` — expected: mã Rust đúng định dạng.

## Suggested Review Order

**Public API contract**

- Tách mapping giúp code pairing công khai dễ kiểm tra và kiểm thử độc lập.
  [`handlers.rs:103`](../../src-tauri/src/interface/http_server/handlers.rs#L103)

- Mã mới mô tả khả năng xác nhận pairing, không lộ channel nội bộ.
  [`handlers.rs:107`](../../src-tauri/src/interface/http_server/handlers.rs#L107)

**Regression protection**

- Khóa HTTP 503, code mới và message không chứa thuật ngữ implementation.
  [`handlers.rs:238`](../../src-tauri/src/interface/http_server/handlers.rs#L238)

- Bảo đảm các mapping pairing còn lại không đổi contract.
  [`handlers.rs:252`](../../src-tauri/src/interface/http_server/handlers.rs#L252)
