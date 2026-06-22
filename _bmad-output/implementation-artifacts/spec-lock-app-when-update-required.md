---
title: 'Khóa toàn bộ ứng dụng khi có phiên bản mới'
type: 'bugfix'
created: '2026-08-18'
status: 'done'
baseline_commit: '8e443f3'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/4-7-create-auto-update-popup-ui.md'
  - '{project-root}/_bmad-output/implementation-artifacts/spec-auto-check-update-on-app-info-open.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Ứng dụng có thể bỏ lỡ event cập nhật được backend phát quá sớm lúc khởi động; kết quả kiểm tra thủ công cũng chỉ nằm trong modal Thông tin. Vì vậy một phiên bản cũ vẫn có thể tiếp tục sử dụng các action bình thường.

**Approach:** Biến trạng thái “có phiên bản mới” thành gate toàn cục tại `AppLayout`. Listener được đăng ký trước khi layout chủ động kiểm tra lại; mọi kết quả kiểm tra có bản mới đều được phát lên cùng kênh sự kiện để gate chuyển sang màn hình bắt buộc cập nhật và loại bỏ toàn bộ UI thao tác phía sau.

## Boundaries & Constraints

**Always:** Khóa tất cả route, nút, dialog và action ngay khi xác nhận có bản mới; chỉ hiển thị `ForcedUpdateModal` trong trạng thái bắt buộc cập nhật; mọi caller của `checkForUpdates` phải kích hoạt cùng gate; giữ cơ chế tự cài đặt, retry, restart và quit hiện có; bảo toàn các thay đổi chưa commit của người dùng.

**Ask First:** Thay đổi chính sách fail-open khi máy chủ cập nhật không truy cập được; thay đổi backend updater hoặc định dạng event/DTO; thêm test framework hay dependency.

**Never:** Cho phép dismiss/bỏ qua bản cập nhật; chỉ khóa riêng modal Thông tin; phụ thuộc duy nhất vào event startup; sửa các thay đổi version/config và UI đang dở của người dùng.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|---------------|----------------------------|----------------|
| Startup có bản mới | Event startup đến sớm hoặc check chủ động trả bản mới | Gate hiển thị `ForcedUpdateModal`; `Outlet` và dialog thường không được render | Kết quả trực tiếp bù cho event có thể bị lỡ |
| Kiểm tra thủ công có bản mới | Bất kỳ caller nào gọi `checkForUpdates` và nhận `update_available: true` | Kết quả được phát lên kênh toàn cục và khóa app | Không phụ thuộc caller biết về `AppLayout` |
| Không có bản mới | Check trả `update_available: false` | Ứng dụng hoạt động bình thường | Không phát event bắt buộc cập nhật |
| Check thất bại | Mất mạng hoặc updater trả lỗi | Giữ chính sách fail-open hiện tại | Không suy diễn có bản mới |
| Event và kết quả cùng đến | Listener nhận event và check trả cùng version | Gate vẫn ổn định, không tạo nhiều modal/cài đặt song song | Cập nhật state idempotent |

</frozen-after-approval>

## Code Map

- `src/components/AppLayout/AppLayout.tsx` — điểm gate toàn cục, listener update và cây UI được phép render.
- `src/services/update-service.ts` — cầu nối Tauri; phát kết quả kiểm tra có bản mới lên kênh sự kiện chung.
- `src/components/ForcedUpdateModal.tsx` — màn hình khóa/cài đặt hiện có; tái sử dụng, không dự kiến sửa.
- `src/pages/printer/components/AppInfoModal.tsx` — caller kiểm tra thủ công; tự động hưởng cơ chế phát sự kiện mới, giữ nguyên thay đổi của người dùng.

## Tasks & Acceptance

**Execution:**
- [x] `src/services/update-service.ts` — sau khi command kiểm tra trả bản mới, phát `update-available` để mọi nguồn kiểm tra cùng kích hoạt gate.
- [x] `src/components/AppLayout/AppLayout.tsx` — đăng ký listener trước, chủ động kiểm tra lúc mount, xử lý idempotent, và chỉ render `ForcedUpdateModal` khi bị khóa.

**Acceptance Criteria:**
- Given app khởi động bằng phiên bản cũ, when event startup bị phát trước khi frontend sẵn sàng, then lần kiểm tra chủ động vẫn phát hiện và khóa toàn ứng dụng.
- Given bất kỳ kiểm tra thủ công nào phát hiện bản mới, when kết quả trả về, then `AppLayout` chuyển sang trạng thái bắt buộc cập nhật.
- Given trạng thái bắt buộc cập nhật, when React render cây ứng dụng, then route hiện tại, pair dialog và mọi action bình thường không còn được render; người dùng chỉ thao tác được với luồng cập nhật.
- Given không có bản mới hoặc check lỗi, when kiểm tra hoàn tất, then ứng dụng tiếp tục hoạt động theo chính sách fail-open hiện tại.

## Spec Change Log

## Design Notes

Listener phải được đăng ký trước lần kiểm tra chủ động. `checkForUpdates` phát event theo mô hình fan-out để lần kiểm tra từ modal Thông tin hoặc caller tương lai không cần truyền callback xuyên qua cây component. `AppLayout` vẫn xử lý trực tiếp kết quả trả về để không phụ thuộc việc giao event bất đồng bộ.

## Verification

**Commands:**
- `pnpm exec eslint src/components/AppLayout/AppLayout.tsx src/services/update-service.ts` — không có lỗi lint trong file thay đổi.
- `pnpm run build:web` — TypeScript và Vite build thành công.
- `git diff --check` — không có lỗi whitespace mới.

**Manual checks:**
- Chạy app version 1.0.0 với metadata update mới hơn; xác nhận chỉ còn màn hình bắt buộc cập nhật và không thể truy cập action phía sau.
- Trả kết quả không có update hoặc mô phỏng check lỗi; xác nhận app vẫn vào màn hình chính.

## Suggested Review Order

**Phát hiện và truyền trạng thái cập nhật**

- Gate gom event và kết quả trực tiếp thành một chuyển trạng thái idempotent.
  [`AppLayout.tsx:20`](../../src/components/AppLayout/AppLayout.tsx#L20)

- Subscriber nội bộ giữ fan-out hoạt động khi Tauri event setup thất bại.
  [`update-service.ts:24`](../../src/services/update-service.ts#L24)

- Mọi lần kiểm tra có bản mới đều thông báo gate toàn cục.
  [`update-service.ts:57`](../../src/services/update-service.ts#L57)

**Khóa giao diện**

- Early return loại bỏ toàn bộ route và dialog khi bắt buộc cập nhật.
  [`AppLayout.tsx:59`](../../src/components/AppLayout/AppLayout.tsx#L59)
