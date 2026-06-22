---
title: 'Trả 204 No Content từ API hủy lệnh in'
type: 'refactor'
created: '2026-08-28'
status: 'done'
baseline_commit: '38a8d12ba1679af29d43a9eb21af1f85758f6a37'
context:
  - '{project-root}/CLAUDE.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Endpoint `POST /api/v1/jobs/cancel` đang trả `{ slip_id, cancelled }`, trong khi frontend không cần dữ liệu phản hồi và contract OpenAPI mong muốn chỉ thể hiện request payload.

**Approach:** Khi use case xử lý thành công, endpoint trả `204 No Content`; giữ nguyên request `{ slip_id }`, Bearer authentication, route, logic hủy và error response hiện hành.

## Boundaries & Constraints

**Always:** Xóa DTO response không còn dùng; bảo đảm status thành công là `204` và response body rỗng; không làm mất việc chờ `spawn_blocking` và xử lý kết quả use case trước khi phản hồi.

**Ask First:** Mọi thay đổi sang fire-and-forget, đổi endpoint/request/auth, hoặc bỏ qua lỗi do use case trả về.

**Never:** Không đổi `CancelPrintJobUseCase`, persistence/event flow, quy tắc chọn job theo `slip_id`, error mapping hoặc các thay đổi chưa commit ngoài phạm vi response contract.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|---------------|----------------------------|----------------|
| Hủy được một hoặc nhiều job | Bearer token và `slip_id` hợp lệ | `204 No Content`, body rỗng | Không áp dụng |
| Không có job có thể hủy | Use case trả `Ok(0)` | `204 No Content`, body rỗng | Không coi là lỗi |
| Use case thất bại | Repository/event store/use case trả lỗi | Giữ status và `{ code, message }` hiện hành | Qua `map_app_error` |

</frozen-after-approval>

## Code Map

- `src-tauri/src/interface/http_server/handlers.rs` — định nghĩa DTO response và chuyển kết quả use case thành HTTP response.
- `src-tauri/src/interface/http_server/router.rs` — xác nhận route vẫn trỏ đến `cancel_slip` và tiếp tục dùng auth middleware.

## Tasks & Acceptance

**Execution:**

- [x] `src-tauri/src/interface/http_server/handlers.rs` — bỏ `CancelSlipResponse`, đổi success mapping sang `StatusCode::NO_CONTENT`, giữ nguyên error propagation.
- [x] `src-tauri/src/interface/http_server/handlers.rs` — bổ sung regression tests tại response-mapping seam cho `Ok(0)`, `Ok(n)` và lỗi use case; xác nhận status `204`, body rỗng và error mapping không đổi.
- [x] Rà source và chạy Rust quality gates để xác nhận không còn DTO response hoặc warning/compile error do refactor.

**Acceptance Criteria:**

- Given request cancel hợp lệ và use case trả `Ok(n)`, when handler hoàn tất, then HTTP response là `204 No Content` và không có body với mọi giá trị `n`, kể cả `0`.
- Given use case trả lỗi, when handler xử lý kết quả, then lỗi vẫn được chuyển qua `map_app_error` như trước.
- Given route và auth hiện tại, when refactor hoàn tất, then `/api/v1/jobs/cancel`, request `{ slip_id }` và Bearer authentication không thay đổi.

## Spec Change Log

- **2026-08-28 — Review loop 1 (bad_spec):** I/O matrix có các nhánh success-zero, success-nonzero và error nhưng kế hoạch ban đầu không yêu cầu regression test. Đã thêm task test response-mapping seam để tránh contract `204` bị hồi quy mà full suite vẫn xanh. **KEEP:** handler vẫn chờ `spawn_blocking`; mọi `Ok(usize)` map sang `StatusCode::NO_CONTENT`; lỗi tiếp tục qua `map_app_error`; request/route/auth giữ nguyên.

## Verification

**Commands:**

- `rg -n "CancelSlipResponse|cancelled: usize" src-tauri/src/interface/http_server` — expected: không có kết quả.
- `cargo fmt --check --manifest-path src-tauri/Cargo.toml` — expected: formatting hợp lệ.
- `cargo check --manifest-path src-tauri/Cargo.toml` — expected: backend biên dịch thành công.
- `cargo test --manifest-path src-tauri/Cargo.toml` — expected: toàn bộ Rust tests vượt qua.

## Suggested Review Order

**HTTP contract**

- Handler vẫn chờ use case trước khi tạo response thành công.
  [`handlers.rs:202`](../../src-tauri/src/interface/http_server/handlers.rs#L202)

- Response seam bỏ payload và giữ nguyên error mapping.
  [`handlers.rs:222`](../../src-tauri/src/interface/http_server/handlers.rs#L222)

**Regression coverage**

- Helper kiểm tra trực tiếp status `204` và body rỗng.
  [`handlers.rs:261`](../../src-tauri/src/interface/http_server/handlers.rs#L261)

- Tests khóa cả success-zero, success-nonzero và error path.
  [`handlers.rs:324`](../../src-tauri/src/interface/http_server/handlers.rs#L324)
