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

**Approach:** Đổi API error code công khai thành `pairing_confirmation_unavailable`, diễn đạt rằng ứng dụng desktop hiện không thể tiếp nhận yêu cầu xác nhận pairing; giữ nguyên HTTP 503 và đồng bộ biến thể lỗi nội bộ thành `PairError::PairingConfirmationUnavailable`.

## Boundaries & Constraints

**Always:** Giữ response JSON `{ "code", "message" }`; giữ HTTP 503; tên code phải ổn định, machine-readable và mô tả ngữ nghĩa bên ngoài thay vì cơ chế `mpsc` bên trong.

**Ask First:** Bất kỳ thay đổi tiếp theo nào đối với tên enum nội bộ, quy trình pairing, timeout, CORS hoặc cơ chế cấp token; lần đổi sang `PairingConfirmationUnavailable` đã được người dùng phê duyệt ngày 2026-08-24.

**Never:** Duy trì alias cho mã cũ; thay đổi các mã `user_denied`, `pair_timeout`; mở rộng sang chuẩn hóa toàn bộ error response API.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Desktop confirmation channel unavailable | `request_pair` trả `PairError::PairingConfirmationUnavailable` | HTTP 503 với JSON code `pairing_confirmation_unavailable` | Message không nhắc tới subscriber/channel nội bộ |
| Các lỗi pairing khác | User từ chối, timeout hoặc backend error | Giữ nguyên status, code và message hiện tại | Không regression API contract ngoài mã được đổi |

</frozen-after-approval>

## Code Map

- `src-tauri/src/interface/http_server/handlers.rs` — chuyển `PairError::PairingConfirmationUnavailable` thành HTTP error response công khai.
- `src-tauri/src/application/ports/api_token_port.rs` — định nghĩa và message của biến thể lỗi pairing nội bộ.
- `src-tauri/src/infrastructure/persistence/api_token_repository.rs` — phát sinh biến thể lỗi khi UI sink thiếu hoặc đã đóng.

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/src/interface/http_server/handlers.rs` — đổi public error code và message của nhánh `PairError::PairingConfirmationUnavailable`.
- [x] `src-tauri/src/interface/http_server/handlers.rs` — bổ sung kiểm thử khóa HTTP status, code và message của mapping để tránh regression.
- [x] `src-tauri/src/application/ports/api_token_port.rs` — đổi enum nội bộ và Display message sang ngôn ngữ pairing confirmation.
- [x] `src-tauri/src/infrastructure/persistence/api_token_repository.rs` — cập nhật toàn bộ nơi khởi tạo biến thể lỗi mới.

**Acceptance Criteria:**
- Given desktop confirmation channel chưa được đăng ký hoặc đã đóng, when web FE gọi `POST /api/v1/pair`, then API trả HTTP 503 với JSON code `pairing_confirmation_unavailable`.
- Given cùng lỗi trên, when FE đọc message, then message mô tả không thể yêu cầu xác nhận pairing và không chứa thuật ngữ `subscriber`.
- Given các nhánh pairing còn lại, when mapping lỗi, then contract hiện tại không thay đổi.

## Spec Change Log

- 2026-08-24 — Người dùng yêu cầu đổi toàn bộ tên `NoUiSubscriber` cho đồng bộ. Mở rộng intent sang `PairingConfirmationUnavailable`, cập nhật Code Map/checklist và loại bỏ thuật ngữ subscriber khỏi Display message; giữ nguyên HTTP 503 và public code.

## Verification

**Commands:**
- `cargo test interface::http_server::handlers --lib` — expected: kiểm thử mapping pairing pass.
- `cargo test infrastructure::persistence::api_token_repository --lib` — expected: kiểm thử token repository pass.
- `git diff --check` — expected: diff không có whitespace error; formatting debt toàn crate được theo dõi riêng trong `deferred-work.md`.

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
