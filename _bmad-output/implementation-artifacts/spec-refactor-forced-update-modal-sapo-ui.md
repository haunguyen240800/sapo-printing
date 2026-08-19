---
title: 'Refactor ForcedUpdateModal sang Sapo UI Components'
type: 'refactor'
created: '2026-08-19'
status: 'done'
baseline_commit: '9eae098'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/spec-lock-app-when-update-required.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** `ForcedUpdateModal` đang tự dựng overlay, card, typography, error box và action layout bằng thẻ HTML cùng inline style, không đồng nhất với hệ thống giao diện của ứng dụng.

**Approach:** Thay lớp trình bày bằng các component có sẵn trong `@sapo/ui-components`, sử dụng `Modal` làm container bắt buộc cập nhật và các primitive Sapo cho nội dung/trạng thái/action, trong khi giữ nguyên toàn bộ state machine và lệnh updater hiện tại.

## Boundaries & Constraints

**Always:** Dùng `Modal`, `BlockStack`, `InlineStack`, `Text`, `Icon`, `Spinner`, `Banner` và action footer từ `@sapo/ui-components`; modal luôn mở và mọi callback đóng/backdrop/Escape đều không được phép dismiss; giữ tự động cài đặt khi mount, trạng thái installing/agent/ready/error, retry, restart, quit sau ba lần lỗi và hiển thị version; bảo toàn các thay đổi chưa commit của người dùng.

**Ask First:** Thay đổi hành vi updater, số lần retry, nội dung nghiệp vụ, backend command/event, hoặc cần thêm dependency/component dùng chung.

**Never:** Giữ overlay/card/error/action bằng thẻ HTML và inline style; thêm khả năng bỏ qua/đóng modal; sửa `AppLayout`, update service, version/config hoặc các file UI đang dở khác.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|---------------|----------------------------|----------------|
| Đang cài | `installing` hoặc `agent` | Modal Sapo hiển thị spinner và đúng thông điệp; không có action dismiss | Không cho đóng bằng X, backdrop hoặc Escape |
| Sẵn sàng | Nhận `update-ready-to-apply` | Footer Modal có action chính “Khởi động lại” | Giữ fallback reload hiện có nếu restart command lỗi |
| Cài lỗi | `installUpdate` reject | Banner critical hiển thị lỗi; footer có “Thử lại” | Chuyển lại installing khi retry |
| Lỗi lặp lại | Số lần lỗi đạt 3 | Ngoài retry, footer hiển thị “Thoát ứng dụng” | Giữ command quit hiện có |
| Có version | Current/new version khả dụng | Nội dung Modal hiển thị đúng hai version | Thiếu version thì không render dòng tương ứng |

</frozen-after-approval>

## Code Map

- `src/components/ForcedUpdateModal.tsx` — component duy nhất cần refactor; chứa UI và state machine bắt buộc cập nhật.
- `src/components/AppLayout/AppLayout.tsx` — gate chỉ render modal khi có update; context tham chiếu, không sửa.
- `src/services/update-service.ts` — command/event updater được component gọi; context tham chiếu, không sửa.
- `node_modules/@sapo/ui-components/dist/index.d.ts` — contract component/action được dùng để bảo đảm props hợp lệ.

## Tasks & Acceptance

**Execution:**
- [x] `src/components/ForcedUpdateModal.tsx` — thay toàn bộ JSX HTML/style thủ công bằng Sapo UI components và Modal footer actions, giữ nguyên logic updater.

**Acceptance Criteria:**
- Given forced update được kích hoạt, when component render, then UI sử dụng `Modal` và primitive từ `@sapo/ui-components`, không còn inline style hoặc layout HTML thủ công.
- Given người dùng bấm X, backdrop hoặc Escape, when `Modal.onClose` chạy, then modal vẫn mở và không có action ứng dụng nào được mở khóa.
- Given từng trạng thái installing, agent, ready và error, when state thay đổi, then thông điệp cùng action tương ứng vẫn hoạt động như trước.
- Given cài đặt lỗi ít hơn hoặc từ ba lần trở lên, when footer render, then retry luôn có sẵn và quit chỉ xuất hiện từ lần lỗi thứ ba.

## Spec Change Log

## Design Notes

`Modal` là controlled component: truyền `open` cố định và `onClose` no-op để giữ chính sách bắt buộc cập nhật dù người dùng tương tác với nút X, backdrop hay Escape. Action được khai báo qua `primaryAction`/`secondaryActions`, tránh tự dựng hàng nút. Error dùng `Banner tone="critical"`; loading dùng `Spinner` cùng typography/layout token của Sapo.

## Verification

**Commands:**
- `pnpm exec eslint src/components/ForcedUpdateModal.tsx` — không có lỗi lint.
- `pnpm run build:web` — TypeScript và Vite build thành công.
- `git diff --check -- src/components/ForcedUpdateModal.tsx` — không có lỗi whitespace mới.

**Manual checks:**
- Kích hoạt forced update và thử X, backdrop, Escape; modal không đóng.
- Kiểm tra các trạng thái installing, ready, error/retry và quit sau ba lần lỗi.

## Suggested Review Order

- Controlled Sapo Modal giữ gate bắt buộc và vô hiệu hóa mọi đường dismiss.
  [`ForcedUpdateModal.tsx:110`](../../src/components/ForcedUpdateModal.tsx#L110)

- Footer actions ánh xạ ready/error/retry threshold mà không đổi state machine.
  [`ForcedUpdateModal.tsx:97`](../../src/components/ForcedUpdateModal.tsx#L97)

- Sapo primitives thay toàn bộ layout, typography, loading và error thủ công.
  [`ForcedUpdateModal.tsx:126`](../../src/components/ForcedUpdateModal.tsx#L126)
