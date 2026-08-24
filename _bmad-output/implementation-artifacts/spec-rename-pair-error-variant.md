---
title: 'Đồng bộ tên biến thể lỗi xác nhận pairing'
type: 'refactor'
created: '2026-08-24'
status: 'done'
route: 'one-shot'
---

# Đồng bộ tên biến thể lỗi xác nhận pairing

## Intent

**Problem:** Tên enum nội bộ vẫn mô tả cơ chế subscriber trong khi public API đã dùng ngôn ngữ nghiệp vụ về khả năng xác nhận pairing.

**Approach:** Đổi biến thể nội bộ thành `PairingConfirmationUnavailable` tại định nghĩa và mọi nơi sử dụng, đồng bộ Display message, test và tài liệu; giữ nguyên HTTP 503 cùng public code `pairing_confirmation_unavailable`.

## Suggested Review Order

**Application error contract**

- Tên biến thể và Display message cùng diễn đạt trạng thái xác nhận pairing.
  [`api_token_port.rs:42`](../../src-tauri/src/application/ports/api_token_port.rs#L42)

**Infrastructure propagation**

- Cả hai nhánh UI sink thiếu hoặc đóng đều tạo biến thể mới.
  [`api_token_repository.rs:101`](../../src-tauri/src/infrastructure/persistence/api_token_repository.rs#L101)

**HTTP contract and tests**

- Mapping tiếp nhận enum mới nhưng giữ nguyên HTTP 503 và public code.
  [`handlers.rs:107`](../../src-tauri/src/interface/http_server/handlers.rs#L107)

- Test khóa contract bên ngoài sau refactor tên nội bộ.
  [`handlers.rs:238`](../../src-tauri/src/interface/http_server/handlers.rs#L238)

**Audit trail**

- Spec trước ghi nhận việc người dùng phê duyệt mở rộng phạm vi.
  [`spec-rename-pairing-confirmation-unavailable-error.md:55`](spec-rename-pairing-confirmation-unavailable-error.md#L55)
