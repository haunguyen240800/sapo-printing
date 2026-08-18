---
title: 'Tập trung cấu hình privileged agent theo một nguồn'
type: 'refactor'
created: '2026-08-18'
status: 'done'
baseline_commit: '56a82040b6c3f749034b13f2d7d88105eb0ee649'
context:
  - '{project-root}/docs/silent-auto-update-privileged-agent.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Các giá trị cấu hình của privileged updater đang được chép tay giữa `tauri.conf.json`, Rust và installer Windows, nên rotate key, đổi endpoint hoặc đổi tên service dễ tạo ra binary và installer không đồng bộ.

**Approach:** Dùng `tauri.conf.json` làm nguồn cho metadata/updater mà Tauri đã sở hữu, truyền chúng vào Rust tại compile time qua `build.rs`; gom hằng số chỉ thuộc agent vào `agent_config.rs`; installer PS1 nhận tham số và NSIS/WiX truyền rõ các giá trị tương ứng.

## Boundaries & Constraints

**Always:** Giữ nguyên hành vi update, xác minh chữ ký, IPC, SCM và fallback UAC hiện có; endpoint và pubkey mà service dùng phải được derive từ `tauri.conf.json`; các module Rust phải import hằng số agent thay vì giữ literal riêng; script tương thích Windows PowerShell 5.1 và ASCII-only; `build.rs` phải báo lỗi build rõ ràng khi cấu hình bắt buộc thiếu hoặc sai kiểu và rerun khi `tauri.conf.json` đổi.

**Ask First:** Bất kỳ thay đổi nào làm đổi giá trị production hiện tại, đổi protocol IPC, đổi tên binary/service đã cài, hoặc yêu cầu migration service cũ.

**Never:** Chép lại pubkey/endpoint/version/productName vào Rust; nhận endpoint, URL tải hoặc pubkey từ IPC runtime; thay đổi business flow của privileged updater; sửa các thay đổi frontend không liên quan đang có trong worktree.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Build hợp lệ | `tauri.conf.json` có version, productName, updater pubkey và endpoint | Rust nhận đúng giá trị tại compile time và Tauri build tiếp tục | N/A |
| Config thiếu/sai | Một trường bắt buộc thiếu, sai kiểu hoặc endpoint rỗng | Build dừng, chỉ rõ trường cấu hình lỗi | Không compile với giá trị mặc định production |
| Installer chạy trực tiếp | PS1 không được truyền tham số | Dùng default hiện tại để tương thích ngược | Validate tham số và báo lỗi nếu agent exe không tồn tại |
| Installer qua NSIS/WiX | Hook truyền service/exe name | Hai installer đăng ký/gỡ đúng cùng service | Giữ graceful fallback hiện có khi đăng ký thất bại |

</frozen-after-approval>

## Code Map

- `src-tauri/tauri.conf.json` — nguồn chuẩn cho productName, version, updater pubkey và endpoint.
- `src-tauri/build.rs` — đọc/validate Tauri config, phát `cargo:rustc-env` và giữ `tauri_build::build()`.
- `src-tauri/Cargo.toml` — build dependency cần để parse JSON.
- `src-tauri/src/infrastructure/platform/agent_config.rs` — nguồn Rust duy nhất cho service name, IPC endpoints, exe names, platform key và metadata chứng thư.
- `src-tauri/src/infrastructure/platform/updater/service_updater.rs` — consumer của updater env và agent constants.
- `src-tauri/src/infrastructure/platform/tls/{ipc.rs,cert_generator.rs,cert_dir.rs}` — consumer của endpoint/CA/data-dir constants.
- `src-tauri/src/bin/sapo_printer_cert_manager.rs` — consumer của service/app exe constants.
- `installers/windows/{register-agent-service.ps1,unregister-agent-service.ps1}` — script nhận tham số có default tương thích ngược.
- `installers/windows/{nsis-hooks.nsh,wix-fragment.wxs}` — truyền explicit service/exe name, ghi chú nguồn Rust canonical.

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/build.rs`, `src-tauri/Cargo.toml` — parse và validate Tauri config, phát compile-time env cho product/version/pubkey/endpoint.
- [x] `src-tauri/src/infrastructure/platform/agent_config.rs`, `platform/mod.rs` — định nghĩa và export các hằng số agent thuần Rust.
- [x] Updater/TLS/cert-manager modules — thay literal thuộc phạm vi bằng import hoặc `env!`, không đổi luồng runtime.
- [x] Hai PS1 và hooks NSIS/WiX — parameter hóa service/exe name và truyền giá trị hiện tại một cách tường minh.
- [x] Tests/build checks — chứng minh config hợp lệ compile và không còn duplicate updater secrets trong Rust.

**Acceptance Criteria:**
- Given pubkey hoặc updater endpoint thay đổi duy nhất trong `tauri.conf.json`, when Rust được rebuild, then service updater compile với giá trị mới mà không sửa file Rust.
- Given một agent constant được dùng ở nhiều module Rust, when constant thay đổi trong `agent_config.rs`, then tất cả consumer dùng cùng giá trị sau rebuild.
- Given installer hook chạy register/unregister, when PS1 được gọi, then service name và agent exe name được truyền rõ hoặc dùng default tương thích ngược.
- Given codebase được tìm kiếm, when kiểm tra updater pubkey/endpoint literals trong Rust, then không còn bản chép tay production.

## Spec Change Log

## Design Notes

Compile-time env chỉ chứa dữ liệu Tauri đã sở hữu; agent constants không phụ thuộc Tauri và không cần code generation. Installer boundary vẫn explicit vì PS1/NSIS/WiX không thể import Rust module tại runtime; comment nguồn và tham số hóa làm điểm lệch dễ kiểm tra trong release review mà không thêm runtime config có thể bị sửa bởi user thường.

## Verification

**Commands:**
- `rustfmt --edition 2024 --config skip_children=true --check <các file Rust đã thay đổi>` — các file trong scope có formatting hợp lệ; toàn workspace hiện còn baseline chưa format.
- `cargo check --all-targets` — build script và mọi consumer compile.
- `cargo test infrastructure::platform::tls` — test TLS liên quan pass.
- `cargo test infrastructure::platform::updater` — test updater liên quan pass.
- `rg` kiểm tra literal updater/service/IPC — không còn duplicate ngoài nguồn canonical và installer boundary đã ghi chú.

## Suggested Review Order

**Nguồn cấu hình và build contract**

- Entry point đọc, validate và truyền metadata Tauri vào Rust lúc compile.
  [`build.rs:5`](../../src-tauri/build.rs#L5)

- Agent-only constants được gom tại một module canonical.
  [`agent_config.rs:7`](../../src-tauri/src/infrastructure/platform/agent_config.rs#L7)

**Runtime consumers**

- Privileged updater lấy endpoint/pubkey từ compile-time env, không giữ literal.
  [`service_updater.rs:17`](../../src-tauri/src/infrastructure/platform/updater/service_updater.rs#L17)

- Windows Service và relaunch executable dùng chung agent constants.
  [`sapo_printer_cert_manager.rs:20`](../../src-tauri/src/bin/sapo_printer_cert_manager.rs#L20)

- IPC và certificate metadata derive từ cùng agent module.
  [`ipc.rs:15`](../../src-tauri/src/infrastructure/platform/tls/ipc.rs#L15)

**Installer boundary**

- PS1 validate tham số và cập nhật cả service đã tồn tại.
  [`register-agent-service.ps1:6`](../../installers/windows/register-agent-service.ps1#L6)

- NSIS truyền explicit service/exe values tại boundary không import được Rust.
  [`nsis-hooks.nsh:5`](../../installers/windows/nsis-hooks.nsh#L5)

- WiX giữ parity với NSIS cho register/unregister.
  [`wix-fragment.wxs:28`](../../installers/windows/wix-fragment.wxs#L28)

**Metadata consumers**

- HTTP API version derive từ Tauri version thay vì Cargo package version.
  [`http_server.rs:36`](../../src-tauri/src/bootstrap/http_server.rs#L36)

- Tray tooltip derive từ Tauri productName.
  [`tray.rs:19`](../../src-tauri/src/bootstrap/tray.rs#L19)
