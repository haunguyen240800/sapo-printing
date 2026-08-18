---
title: 'Tự kiểm tra cập nhật khi mở modal Thông tin'
type: 'feature'
created: '2026-08-18'
status: 'done'
baseline_commit: 'e69e3ba653d029fb1461a85481477c519b6fb7ca'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/4-7-create-auto-update-popup-ui.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Modal Thông tin hiện yêu cầu người dùng bấm “Kiểm tra phiên bản” và luôn hiển thị banner kể cả khi không có bản cập nhật, tạo thêm thao tác và thông tin thừa.

**Approach:** Tự kiểm tra cập nhật mỗi lần modal được mở. Chỉ hiển thị banner khi đã xác nhận có phiên bản mới; đặt hành động “Cập nhật” ngay trong banner và chỉ giữ nút “Đóng” ở footer.

## Boundaries & Constraints

**Always:** Tái sử dụng `useAppUpdate` và command cập nhật hiện có; kiểm tra lại ở mỗi lần mở modal; giữ luồng tải/cài đặt và khóa đóng modal trong lúc cài đặt; nội dung giao diện bằng tiếng Việt.

**Ask First:** Bất kỳ thay đổi nào đến backend updater, chính sách bắt buộc cập nhật, hoặc API của component dùng chung.

**Never:** Thêm dependency; thay đổi `ForcedUpdateModal`; hiển thị banner cho trạng thái đang kiểm tra hoặc không có phiên bản mới; giữ nút “Kiểm tra phiên bản” thủ công.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Có bản mới | Mở modal, API trả `update_available: true` cùng version | Banner thông báo version mới và nút “Cập nhật” xuất hiện | Giữ nguyên modal để người dùng có thể đóng hoặc cập nhật |
| Đã mới nhất | Mở modal, API trả `update_available: false` | Không hiển thị banner; footer chỉ có “Đóng” | Không cần thông báo |
| Đang kiểm tra | Vừa mở modal, request chưa hoàn tất | Không hiển thị banner hoặc nút kiểm tra thủ công | Modal thông tin vẫn sử dụng bình thường |
| Lỗi kiểm tra | Request kiểm tra thất bại | Không hiển thị banner cập nhật | Không suy diễn rằng có phiên bản mới |
| Đang cài đặt | Người dùng bấm “Cập nhật” trong banner | Hiển thị tiến trình hiện có và ngăn đóng modal | Lỗi cài đặt tiếp tục dùng thông báo lỗi hiện có |

</frozen-after-approval>

## Code Map

- `src/pages/printer/components/AppInfoModal.tsx` — giao diện modal, trigger kiểm tra và điều kiện hiển thị banner/action.
- `src/pages/printer/hooks/useAppUpdate.ts` — state machine và các hàm kiểm tra/cài đặt hiện có; không dự kiến sửa.
- `src/services/update-service.ts` — cầu nối Tauri command hiện có; không dự kiến sửa.

## Tasks & Acceptance

**Execution:**
- [x] `src/pages/printer/components/AppInfoModal.tsx` — gọi `checkUpdate` khi modal mở; bỏ primary action kiểm tra thủ công; chỉ render banner có bản mới (và các trạng thái cài đặt liên quan); đặt nút cập nhật trong banner.

**Acceptance Criteria:**
- Given modal đang đóng, when người dùng mở modal Thông tin, then ứng dụng tự gọi kiểm tra cập nhật mà không cần thao tác bổ sung.
- Given kết quả không có bản mới hoặc đang chờ kết quả, when modal hiển thị, then không có banner cập nhật và không có nút “Kiểm tra phiên bản”.
- Given có bản mới, when kết quả trả về, then banner hiển thị đúng version mới và nút “Cập nhật”; footer chỉ có nút “Đóng”.
- Given người dùng bắt đầu cài đặt, when tiến trình chạy, then feedback tiến trình hiện có vẫn hiển thị và modal không thể đóng.

## Spec Change Log

## Verification

**Commands:**
- `pnpm run build:web` — TypeScript và Vite build thành công.
- `pnpm exec eslint src/pages/printer/components/AppInfoModal.tsx` — không có lỗi lint trong file thay đổi.

**Manual checks:**
- Mở modal ở trường hợp có và không có bản cập nhật; xác nhận banner/action đúng theo ma trận trạng thái.

## Suggested Review Order

- Tự động kiểm tra mỗi lần modal mở và đặt lại ngữ cảnh cài đặt.
  [`AppInfoModal.tsx:34`](../../src/pages/printer/components/AppInfoModal.tsx#L34)

- Chỉ hiển thị banner có bản mới cùng hành động cập nhật tại chỗ.
  [`AppInfoModal.tsx:52`](../../src/pages/printer/components/AppInfoModal.tsx#L52)

- Footer chỉ giữ nút Đóng và chặn mọi đường đóng khi đang cài.
  [`AppInfoModal.tsx:107`](../../src/pages/printer/components/AppInfoModal.tsx#L107)
