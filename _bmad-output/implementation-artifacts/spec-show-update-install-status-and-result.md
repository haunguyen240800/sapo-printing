---
title: 'Hiển thị trạng thái cài đặt và kết quả sau khi khởi động lại'
type: 'bugfix'
created: '2026-08-19'
status: 'done'
baseline_commit: '496255aa024b04420a16b309acc94aa88631b8c7'
context:
  - '{project-root}/_bmad-output/implementation-artifacts/spec-lock-app-when-update-required.md'
  - '{project-root}/_bmad-output/implementation-artifacts/investigations/forced-update-auto-install-investigation.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Đường cập nhật hiện giao cho Windows Service cài `/S`, khiến app biến mất mà không có tiến trình cài đặt hay kết quả rõ ràng. Relaunch còn phụ thuộc thread của service cũ dù installer chủ động dừng service đó.

**Approach:** Không dùng privileged service để cài update ứng dụng. Luôn tải bằng Tauri updater, sau đó chạy Windows installer trong user session với `installMode: passive`: có UAC, thanh tiến trình và tự mở lại app; frontend báo rõ từng giai đoạn và đối chiếu version sau relaunch.

## Boundaries & Constraints

**Always:** Đồng bộ hai UI; khóa action lặp; báo rõ tải xuống/chuẩn bị/mở installer; installer hiển thị UAC và progress; giữ service cho chứng chỉ nhưng không giao nó cài update; bảo toàn diff hiện tại.

**Ask First:** Thêm dependency/test framework; đổi khỏi Tauri updater; tạo updater window/exe riêng; đổi chính sách bắt buộc cập nhật.

**Never:** Gọi đường service `/S`; báo thành công khi version chưa khớp; cho bỏ qua cập nhật bắt buộc; che lỗi bằng reload; hiển thị phần trăm giả.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|---------------|---------------------------|----------------|
| Tải thành công | Có update, user bấm tải | Hiện trạng thái tải; khi xong hiện action cài đặt | Hiện lỗi và cho thử lại |
| Cài thành công | Có `pending_update`, user bấm cài | Hiện “Đang mở trình cài đặt”, sau đó UAC + progress; installer tự mở bản mới | Xóa pending khi đúng version |
| Vẫn là bản cũ | Pending khác version hiện tại | Báo chưa hoàn tất và cho thử lại | Giữ gate bắt buộc |
| Mở installer lỗi | Invoke lỗi trước khi app đóng | App vẫn mở, hiện lỗi và cho thử lại | Không reload webview |

</frozen-after-approval>

## Code Map

- `src-tauri/src/interface/tauri/commands/update_command.rs` — chọn đường tải/cài update.
- `src-tauri/src/infrastructure/platform/updater/update_checker.rs` — Tauri download + installer handle.
- `src-tauri/tauri.conf.json` — Windows `installMode: passive`.
- `src/components/ForcedUpdateModal.tsx` — UI cập nhật bắt buộc.
- `src/pages/printer/hooks/useAppUpdate.ts`, `src/pages/printer/components/AppInfoModal.tsx` — state/UI modal Thông tin.
- `src/services/update-service.ts` — command bridge và pending target.
- `src/components/AppLayout/AppLayout.tsx` — xác nhận sau relaunch.

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/src/interface/tauri/commands/update_command.rs` — bỏ delegate update sang agent; luôn tải vào `pending_update`, emit ready và dùng Tauri `install()` để mở installer passive.
- [x] `src-tauri/tauri.conf.json` — giữ updater `passive` và đặt NSIS `perMachine` để có `/P /R`, UAC, progress và relaunch.
- [x] `src-tauri/src/bin/sapo_printer_cert_manager.rs` — từ chối IPC update qua service; service chỉ còn xử lý trách nhiệm chứng chỉ trong runtime.
- [x] `src/services/update-service.ts` — quản lý pending target version và đối chiếu sau relaunch.
- [x] `src/components/ForcedUpdateModal.tsx` — thêm applying/error, chống double click và bỏ reload.
- [x] `src/pages/printer/hooks/useAppUpdate.ts`, `src/pages/printer/components/AppInfoModal.tsx` — đồng bộ applying/error.
- [x] `src/components/UpdatePopup.tsx` — đồng bộ API updater cũ để build và hành vi lỗi không còn reload che trạng thái.
- [x] `src/components/AppLayout/AppLayout.tsx` — xác nhận pending với version hiện tại và báo kết quả.

**Acceptance Criteria:**
- Given bản mới đã tải, when bấm cài đặt, then UI báo đang cài, khóa action lặp và nói rõ app sẽ tự mở lại.
- Given tải xong, when bấm “Cài đặt và khởi động lại”, then Windows hiển thị UAC và trình cài đặt có progress thay vì cài silent qua service.
- Given installer hoàn tất, when `/R` relaunch app, then bản mới mở trong user session và service chứng chỉ vẫn hoạt động sau update.
- Given mở lại đúng target version, when layout khởi tạo, then báo thành công đúng một lần.
- Given apply/relaunch lỗi hoặc vẫn là bản cũ, when app còn chạy/mở lại, then hiện lỗi có thể thử lại.

## Spec Change Log

- 2026-08-19 — Review phát hiện NSIS mặc định `currentUser`, pending thất bại có thể fail-open, route thường vẫn mount và endpoint service `/S` còn gọi được. Patch đặt NSIS `perMachine`, khóa app ngay từ pending marker, render độc quyền forced-update UI, vô hiệu hóa service update, chống stale/double apply và giữ nguyên các thay đổi version/logging/service registration của người dùng.

## Design Notes

Dấu pending phía UI chỉ ghi nhận yêu cầu; nguồn sự thật là `getVersion()` sau relaunch. `passive` được chọn thay vì `basicUi` để người dùng thấy progress nhưng không phải thao tác lại qua wizard; NSIS `/R` chịu trách nhiệm relaunch. Không giả lập phần trăm tải nếu callback chưa được đưa lên frontend.

## Verification

**Commands:**
- `pnpm exec eslint src/components/ForcedUpdateModal.tsx src/components/AppLayout/AppLayout.tsx src/components/UpdatePopup.tsx src/pages/printer/hooks/useAppUpdate.ts src/pages/printer/components/AppInfoModal.tsx src/services/update-service.ts` — không lỗi lint.
- `pnpm run build:web` — build thành công.
- `cargo check --manifest-path src-tauri/Cargo.toml` — compile thành công.
- `cargo test --manifest-path src-tauri/Cargo.toml` — không regression.
- `git diff --check` — không lỗi whitespace.

**Manual checks:**
- Từ bản cũ, tải update rồi cài: xác nhận UAC, progress, relaunch, success; mô phỏng lỗi/version cũ và xác nhận retry.

**Kết quả tự động:**
- Frontend build, lint và `cargo check` đạt.
- 5 test updater đạt. Full Rust suite: 156 đạt, 4 test có sẵn ngoài updater thất bại ở migration/logger; không liên quan diff này.

## Suggested Review Order

**Luồng cài đặt tương tác**

- Entry point luôn tải bằng Tauri updater, không delegate sang service SYSTEM.
  [`update_command.rs:23`](../../src-tauri/src/interface/tauri/commands/update_command.rs#L23)

- Apply guard mở đúng installer đã tải và chống gọi lặp.
  [`update_command.rs:57`](../../src-tauri/src/interface/tauri/commands/update_command.rs#L57)

- NSIS per-machine bảo đảm UAC; updater passive hiển thị progress và relaunch.
  [`tauri.conf.json:48`](../../src-tauri/tauri.conf.json#L48)

- Privileged agent từ chối đường update silent cũ.
  [`sapo_printer_cert_manager.rs:434`](../../src-tauri/src/bin/sapo_printer_cert_manager.rs#L434)

**Trạng thái và recovery phía UI**

- Pending target khóa app ngay khi lần cài trước chưa hoàn tất.
  [`AppLayout.tsx:20`](../../src/components/AppLayout/AppLayout.tsx#L20)

- Gate chỉ mount forced-update UI, không để action thường chạy phía sau.
  [`AppLayout.tsx:106`](../../src/components/AppLayout/AppLayout.tsx#L106)

- Target version được lưu trước apply và xác minh sau relaunch.
  [`update-service.ts:70`](../../src/services/update-service.ts#L70)

- Forced modal tách rõ tải, sẵn sàng, mở installer và lỗi retry.
  [`ForcedUpdateModal.tsx:44`](../../src/components/ForcedUpdateModal.tsx#L44)

- Modal Thông tin dùng cùng state machine apply/error.
  [`useAppUpdate.ts:57`](../../src/pages/printer/hooks/useAppUpdate.ts#L57)

**Tương thích UI cũ**

- Popup cũ retry đúng giai đoạn thay vì reload che lỗi.
  [`UpdatePopup.tsx:253`](../../src/components/UpdatePopup.tsx#L253)
