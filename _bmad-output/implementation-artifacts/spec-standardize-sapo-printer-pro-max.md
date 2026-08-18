---
title: 'Chuẩn hóa tên sản phẩm Sapo Printer Pro Max'
type: 'chore'
created: '2026-08-18'
status: 'done'
baseline_commit: '832ce8c075b41d172baf2c5124f514dd92773e83'
context:
  - '{project-root}/CLAUDE.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Tên ứng dụng hiện bị trộn giữa `sapo-printer`, `SAPO Printer`, `Sapo Printer` và `Sapo Printer Pro Max` trong metadata, UI, release script, installer và tài liệu. Đặc biệt, bundle macOS được tạo theo `productName` mới nhưng hook cài đặt vẫn trỏ tới `/Applications/Sapo Printer.app`, có thể làm cài/gỡ privileged agent thất bại.

**Approach:** Dùng `Sapo Printer Pro Max` làm tên sản phẩm hiển thị chính thức và `sapo-printer-pro-max` làm package slug. Cập nhật mọi bề mặt build/runtime và tài liệu hiện hành, đồng thời giữ các định danh kỹ thuật đã phát hành để đảm bảo tương thích nâng cấp và dữ liệu người dùng.

## Boundaries & Constraints

**Always:** Tên hiển thị mới phải viết đúng `Sapo Printer Pro Max`; npm/Rust package slug là `sapo-printer-pro-max`; macOS bundle path phải khớp `productName`; release script phải lấy `productName` từ `tauri.conf.json` thay vì lặp lại chuỗi hardcode; lockfile phải phản ánh manifest; các tài liệu hướng dẫn installer phải dùng đúng tên binary/service thực tế.

**Ask First:** Dừng và hỏi trước nếu việc build chứng minh Tauri thay đổi tên executable chính theo package name, hoặc nếu cần migration/xóa identity đã tồn tại trên máy người dùng.

**Never:** Không đổi `com.sapo.printer`, tên executable `sapo-printer`, crate library `sapo_printer`, `SapoPrinterAgent`, `sapo-printer-cert-manager`, IPC endpoint, repository URL, thư mục dữ liệu, Linux group/service filename, tên CA/certificate hoặc endpoint updater. Không chỉnh sửa các `_bmad-output` lịch sử ngoài spec này chỉ để thay thương hiệu.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|---------------|----------------------------|----------------|
| Build Windows | `productName` và version hợp lệ | Release script tìm đúng installer `Sapo Printer Pro Max_<version>_x64-setup.exe` | Báo rõ thiếu `productName`, version hoặc artifact |
| Install macOS | Bundle nằm trong `/Applications` | Hook và launchd plist gọi agent trong `Sapo Printer Pro Max.app` | Script dừng an toàn nếu agent không tồn tại |
| Upgrade bản cũ | Có dữ liệu/service/certificate theo identity cũ | Bản mới tiếp tục dùng và cập nhật được dữ liệu/service hiện có | Không tạo identity hoặc data directory song song |

</frozen-after-approval>

## Code Map

- `package.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `_bmad/bmm/config.yaml` -- package/project metadata.
- `src-tauri/tauri.conf.json`, `index.html`, `src/pages/printer/components/AppInfoModal.tsx` -- nguồn tên hiển thị và bundle.
- `scripts/release.ps1`, `scripts/release.sh` -- xác định artifact release/updater.
- `installers/windows/*`, `installers/macos/*`, `installers/linux/*` -- đường dẫn bundle và chuỗi hiển thị installer/agent.
- `README.md`, `CLAUDE.md`, `docs/**/*.md`, `installers/README.md` -- tài liệu hiện hành; phân biệt tên thương hiệu với identifier/command ổn định.

## Tasks & Acceptance

**Execution:**
- [x] Chuẩn hóa package/project metadata thành `sapo-printer-pro-max` và đồng bộ lockfile.
- [x] Xác nhận mọi title/UI metadata dùng chính xác `Sapo Printer Pro Max`, không để chuỗi tên cũ ở bề mặt người dùng.
- [x] Sửa release script để đọc `productName` và `version` từ Tauri config, có validation rõ ràng.
- [x] Cập nhật macOS bundle paths và các display string của installer/service trên Windows, macOS, Linux.
- [x] Cập nhật tài liệu hiện hành theo tên mới, giữ nguyên mọi identifier, command và path thuộc danh sách tương thích.

**Acceptance Criteria:**
- Given repository sau thay đổi, when quét các file build/runtime, then không còn tên sản phẩm hiển thị `Sapo Printer`/`SAPO Printer` đứng độc lập ngoài các identity được bảo lưu.
- Given manifest npm và Cargo, when đọc package metadata, then cả hai dùng `sapo-printer-pro-max` và Cargo lock khớp.
- Given cấu hình Tauri, when chạy release script, then tên artifact được suy ra từ `productName` thay vì hardcode ở nhiều nơi.
- Given macOS installer hooks, when tra đường dẫn agent, then tất cả trỏ tới `/Applications/Sapo Printer Pro Max.app`.
- Given các identifier ổn định, when so sánh trước và sau, then chúng không bị đổi.

## Spec Change Log

## Design Notes

Tên sản phẩm và technical identity phục vụ hai mục đích khác nhau. Việc giữ identifier/executable/data paths cũ là chủ ý tương thích, không phải sót đổi tên; chỉ metadata package và các chuỗi/bundle path gắn trực tiếp với thương hiệu mới được chuẩn hóa.

## Verification

**Commands:**
- `pnpm run build:web` -- TypeScript và frontend build thành công.
- `cargo check --manifest-path src-tauri/Cargo.toml --bins` -- cả app và privileged agent resolve đúng package metadata.
- `cargo metadata --manifest-path src-tauri/Cargo.toml --no-deps` -- package name và binary names đúng ma trận đặt tên.
- `rg` audit theo tên cũ/mới -- chỉ còn các technical identity được bảo lưu.

**Manual checks (if no CLI):**
- Đối chiếu tên installer do Tauri sinh với cách hai release script dựng đường dẫn.

## Suggested Review Order

**Canonical product identity**

- Nguồn tên hiển thị trung tâm quyết định bundle và installer artifact.
  [`tauri.conf.json:3`](../../src-tauri/tauri.conf.json#L3)

- npm và Rust package cùng dùng canonical technical slug.
  [`package.json:2`](../../package.json#L2)
  [`Cargo.toml:2`](../../src-tauri/Cargo.toml#L2)

**Release artifact derivation**

- PowerShell đọc và kiểm tra product metadata trước khi tìm installer.
  [`release.ps1:30`](../../scripts/release.ps1#L30)

- Git Bash dùng cùng nguồn JSON, tránh hardcode tên sản phẩm.
  [`release.sh:37`](../../scripts/release.sh#L37)

**Installer and upgrade compatibility**

- macOS hook dùng bundle mới và kiểm tra agent trước khi đổi trust store.
  [`postinstall.sh:5`](../../installers/macos/postinstall.sh#L5)

- Uninstall hỗ trợ agent trong bundle cũ để dọn certificate an toàn.
  [`preremove.sh:5`](../../installers/macos/preremove.sh#L5)

- launchd gọi executable bên trong bundle mang tên mới.
  [`com.sapo.printer.cert-manager.plist:9`](../../installers/macos/com.sapo.printer.cert-manager.plist#L9)

- Windows cập nhật display metadata nhưng luôn khởi động lại service.
  [`register-agent-service.ps1:19`](../../installers/windows/register-agent-service.ps1#L19)

**Documentation and follow-up**

- Hướng dẫn installer phản ánh đúng bundle và helper binary thực tế.
  [`installers/README.md:1`](../../installers/README.md#L1)

- Các giới hạn macOS packaging ngoài phạm vi được ghi lại rõ ràng.
  [`deferred-work.md:3`](deferred-work.md#L3)
