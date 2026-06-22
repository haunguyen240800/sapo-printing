# Investigation: ForcedUpdateModal tự tải và cài đặt bản mới

## Hand-off Brief

1. **What happened.** Người dùng quan sát thấy ứng dụng tự tải/cài bản mới ngay khi `ForcedUpdateModal` xuất hiện; source hiện có bằng chứng trực tiếp về lời gọi cài đặt khi mount.
2. **Where the case stands.** Active — đã xác định stronghold tại `ForcedUpdateModal.tsx:72`; cần lần theo `installUpdate` qua service và Tauri command để mô tả đầy đủ cơ chế.
3. **What's needed next.** Truy vết caller chain frontend → IPC → updater backend để xác nhận tải, cài và restart diễn ra ở đâu.

## Case Info

| Field            | Value |
| ---------------- | ----- |
| Ticket           | N/A |
| Date opened      | 2026-08-19 |
| Status           | Active |
| System           | Windows desktop, Tauri, React/TypeScript, Rust |
| Evidence sources | `src/components/ForcedUpdateModal.tsx`, source updater và version control |

## Problem Statement

Người dùng hỏi tại sao khi có phiên bản mới, `ForcedUpdateModal` tự tải xong rồi cài đặt lại.

## Evidence Inventory

| Source | Status | Notes |
| ------ | ------ | ----- |
| `src/components/ForcedUpdateModal.tsx` | Available | Component gọi `runInstall()` trong effect mount |
| `src/services/update-service.ts` | Available | Chưa truy vết trong Outcome 1 |
| Tauri update command/backend | Available | Đã định vị command, agent client và updater fallback; chưa đọc caller chain chi tiết |
| Version control | Available | `git blame` quy hành vi auto-install về commit `56a8204` |
| Targeted tests | Missing | Không tìm thấy test cho forced modal hoặc install command |
| Runtime logs | Missing | Không có log/diagnostic artifact trong repository |
| Issue tracker | Missing | Không có ticket ID hoặc connector evidence trong input |

## Investigation Backlog

| # | Path to Explore | Priority | Status | Notes |
| - | --------------- | -------- | ------ | ----- |
| 1 | Trace `runInstall` đến frontend update service | High | In Progress | Đã định vị `invoke("install_update")`; cần đọc context |
| 2 | Trace Tauri command đến agent/updater | High | In Progress | Đã định vị agent và fallback; cần đọc context |
| 3 | Kiểm tra commit tạo auto-install-on-mount | Medium | Done | `git blame` xác nhận commit `56a8204` |

## Timeline of Events

| Time | Event | Source | Confidence |
| ---- | ----- | ------ | ---------- |
| Component mount | Effect gọi `runInstall()` | `src/components/ForcedUpdateModal.tsx:72` | Confirmed |
| Sau đó | `runInstall` gọi `installUpdate()` | `src/components/ForcedUpdateModal.tsx:42-46` | Confirmed |

## Confirmed Findings

### Finding 1: Component chủ động bắt đầu cài đặt khi mount

**Evidence:** `src/components/ForcedUpdateModal.tsx:72`

**Detail:** Effect thiết lập event listeners rồi gọi `runInstall()` mà không chờ thao tác người dùng.

## Deduced Conclusions

### Deduction 1: Auto-install bắt nguồn từ frontend modal

**Based on:** Finding 1 và `src/components/ForcedUpdateModal.tsx:42-46`

**Reasoning:** Modal mount → `runInstall()` → `installUpdate()`.

**Conclusion:** Việc tự bắt đầu update không phải hành vi tự phát của Sapo `Modal`; component đã chủ động yêu cầu cài đặt.

## Hypothesized Paths

### Hypothesis 1: `installUpdate` thực hiện toàn bộ download/install qua Tauri command

**Status:** Open

**Theory:** Frontend service invoke command Rust, command ủy quyền agent hệ thống hoặc fallback Tauri updater.

**Supporting indicators:** Tên hàm `installUpdate` và kiến trúc updater hiện có.

**Would confirm:** Caller chain trong `src/services/update-service.ts` và Tauri update command.

**Would refute:** Service chỉ cập nhật state hoặc không gọi IPC.

**Resolution:** Chưa truy vết.

## Missing Evidence

| Gap | Impact | How to Obtain |
| --- | ------ | ------------- |
| Caller chain backend | Chưa thể giải thích chính xác tải/cài/restart | Đọc service và Rust command liên quan |
| Runtime logs | Không xác nhận nhánh agent hay fallback trên máy cụ thể | Thu thập tracing updater khi cần |

## Source Code Trace

| Element       | Detail |
| ------------- | ------ |
| Error origin  | Không phải error; trigger tại `src/components/ForcedUpdateModal.tsx:72` |
| Trigger       | `ForcedUpdateModal` mount sau khi gate phát hiện update |
| Condition     | Component được render với forced update active |
| Related files | `src/services/update-service.ts`, Tauri update command, updater backend |

## Conclusion

**Confidence:** Medium

Đã xác nhận modal tự gọi cài đặt khi mount. Cần truy vết backend để kết luận đầy đủ cách tải, cài và restart.

## Recommended Next Steps

### Fix direction

Chưa đề xuất sửa trước khi hoàn tất caller chain; nếu muốn yêu cầu xác nhận, điểm thay đổi dự kiến là bỏ lời gọi `runInstall()` khỏi mount effect và đưa nó vào action người dùng.

### Diagnostic

Đọc service và Rust command updater, sau đó đối chiếu nhánh agent/fallback.

## Reproduction Plan

Render `ForcedUpdateModal` với update hợp lệ; quan sát IPC `install_update` được gọi ngay sau mount mà không click.

## Side Findings

- Không có bằng chứng Sapo `Modal` tự kích hoạt update; trigger nằm trong logic component.

## Follow-up: 2026-08-19

### New Evidence

- Frontend service có `installUpdate()` tại `src/services/update-service.ts:68-69`.
- Tauri command `install_update` được đăng ký tại `src-tauri/src/lib.rs:99` và triển khai tại `src-tauri/src/interface/tauri/commands/update_command.rs:26`.
- Command gọi agent tại `update_command.rs:48`; nếu agent lỗi, gọi updater fallback tại `update_command.rs:70`.
- Hành vi gọi `runInstall()` khi mount xuất hiện từ commit `56a8204` theo `git blame`.

### Additional Findings

Evidence perimeter hiện đủ để truy caller chain tĩnh. Không có test mục tiêu hoặc runtime log trong repository; vì vậy nhánh agent/fallback thực tế trên máy chỉ có thể kết luận từ source, trừ khi thu thập tracing runtime.

### Updated Hypotheses

Hypothesis 1 vẫn Open nhưng có supporting evidence mạnh hơn; cần đọc nội dung bốn file liên quan để Confirm/Refute.

### Backlog Changes

Backlog 1 và 2 chuyển sang In Progress; backlog 3 hoàn tất.

### Updated Conclusion

**Confidence:** Medium. Auto-trigger và các điểm chuyển tiếp đã được định vị; cơ chế chi tiết chưa được truy tuần tự.
