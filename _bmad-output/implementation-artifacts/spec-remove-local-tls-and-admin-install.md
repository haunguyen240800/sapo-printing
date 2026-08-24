---
title: 'Bỏ TLS local và cài đặt không cần quyền admin'
type: 'refactor'
created: '2026-08-24'
status: 'done'
baseline_commit: '210ca5b094ef73b2c575bb6424522c0629ef177b'
context:
  - '{project-root}/CLAUDE.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Local agent hiện phụ thuộc HTTPS certificate tự ký, binary cert-manager, Windows Service và installer per-machine nên việc cài đặt/provision certificate yêu cầu quyền admin. Người dùng muốn webapp gọi trực tiếp `http://127.0.0.1:18901/api/v1/ping` và app có thể cài trong tài khoản hiện tại.

**Approach:** Chuyển Axum local server sang HTTP chỉ bind loopback, loại bỏ toàn bộ mã nguồn/dependency/resource/hook liên quan TLS, CA và cert-manager, đồng thời đóng gói Windows bằng NSIS per-user. Giữ nguyên REST/SSE, xác thực token, CORS và auto-update trong user session.

## Boundaries & Constraints

**Always:** Server chỉ listen trên `127.0.0.1`; cổng ưu tiên vẫn là `18901` và fallback `18902..=18910`; metadata `agent.json`, route `/api/v1/*`, pairing/token, rate limit và SSE giữ nguyên hành vi; các URL HTTPS bên ngoài như updater và document download không bị thay đổi.

**Ask First:** Bất kỳ thay đổi nào làm mất auto-update hiện tại hoặc mở server ra LAN; bất kỳ yêu cầu giữ MSI/per-machine song song với installer không-admin.

**Never:** Không vô hiệu hóa auth/CORS để bù cho việc bỏ TLS; không thay `https://` của API bên ngoài; không giữ lại CA, certificate generation/renewal, trust-store hook, privileged service hay binary cert-manager “để dự phòng”.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| HTTP happy path | `GET http://127.0.0.1:18901/api/v1/ping` khi cổng trống | HTTP 200 với payload ping hiện tại | N/A |
| Preferred port occupied | `18901` đã được process khác bind | Server bind loopback ở cổng đầu tiên trống trong `18902..=18910`, ghi đúng `agent.json` | Log lỗi và tắt web integration nếu hết range |
| Missing legacy cert | Máy mới hoặc cert cũ đã bị xóa | Server vẫn khởi động bình thường | Không đọc/sinh/cài certificate |
| Browser origin | Request từ origin Sapo hợp lệ | CORS/pairing hoạt động như trước qua HTTP loopback | Origin không hợp lệ vẫn bị từ chối |

</frozen-after-approval>

## Code Map

- `src-tauri/src/interface/http_server/{server,bootstrap}.rs` -- khởi động Axum và hiện đang load/hot-reload Rustls certificate.
- `src-tauri/src/bootstrap/http_server.rs` -- wiring server với shared cert directory.
- `src-tauri/src/infrastructure/platform/tls/` -- certificate lifecycle, IPC và port binder; chỉ port binder cần được giữ/chuyển vị trí.
- `src-tauri/src/bin/sapo_printer_cert_manager.rs` -- privileged helper/service cần loại bỏ.
- `src-tauri/Cargo.toml` và `src-tauri/Cargo.lock` -- TLS/cert/service dependencies và binary target.
- `src-tauri/tauri.conf.json` -- cert/service resources, MSI target và NSIS per-machine mode.
- `installers/{windows,macos,linux}/` -- trust-store và service hooks cần loại bỏ hoặc làm sạch.
- `docs/webapp_integration.md` -- hợp đồng tích hợp webapp cần đổi sang `http://127.0.0.1`.

## Tasks & Acceptance

**Execution:**
- [x] Refactor HTTP server bootstrap để không nhận cert path, dùng plain Axum listener trên loopback và giữ port fallback/metadata.
- [x] Di chuyển `port_binder` khỏi namespace TLS; cập nhật imports, logs và tests.
- [x] Xóa module TLS/certificate/IPC, cert-manager binary, privileged updater leftovers và các error/config constant chỉ phục vụ TLS/service.
- [x] Loại bỏ Rust TLS/cert/watch/service dependencies trực tiếp không còn dùng; regenerate lockfile bằng Cargo.
- [x] Xóa installer scripts/hooks/resources cho CA và service; cấu hình Windows NSIS `perUser` và không phát hành MSI yêu cầu elevation.
- [x] Cập nhật tài liệu webapp integration sang HTTP loopback và loại bỏ hướng dẫn cert/trust-store.
- [x] Thêm/cập nhật test chứng minh server khởi động không cần cert và chỉ bind loopback.

**Acceptance Criteria:**
- Given máy không có CA/cert/service, when app khởi động, then local API trả lời qua `http://127.0.0.1:<selected-port>/api/v1/ping`.
- Given build installer Windows mới, when user cài bằng NSIS, then installer không yêu cầu elevation và chỉ ghi tài nguyên theo user.
- Given source tree production, when tìm các cơ chế local TLS/certificate, then không còn cert-manager, trust-store hooks, Rustls local server hoặc certificate lifecycle code.
- Given update/document URLs bên ngoài, when build hoàn tất, then chúng vẫn dùng HTTPS như cấu hình ban đầu.

## Spec Change Log

## Design Notes

Plain HTTP chỉ được chấp nhận cho endpoint loopback. `127.0.0.1` được dùng rõ ràng thay vì hostname public để tránh DNS rebinding và đảm bảo listener không expose ra interface mạng.

## Verification

**Commands:**
- `cargo fmt --all -- --check` -- toàn bộ Rust source đúng format.
- `cargo check --all-targets` -- không còn reference/dependency target cert-manager bị lỗi.
- `cargo test --all-targets` -- unit/integration tests thành công.
- `pnpm run build:web` -- frontend và Tauri config vẫn hợp lệ.
- `rg -n -i "cert-manager|install-ca|uninstall-ca|tls-rustls|RustlsConfig|local.mysapo.net" src-tauri installers` -- không còn cơ chế local TLS/cert trong artifact sản phẩm.

## Suggested Review Order

**Luồng HTTP loopback**

- Entry point khởi động local agent không còn phụ thuộc certificate directory.
  [`http_server.rs:14`](../../src-tauri/src/bootstrap/http_server.rs#L14)

- Plain Axum server dùng listener loopback và không tạo TLS acceptor.
  [`server.rs:42`](../../src-tauri/src/interface/http_server/server.rs#L42)

- Port fallback luôn bind IPv4 loopback trong dải đã định.
  [`port_binder.rs:9`](../../src-tauri/src/infrastructure/platform/port_binder.rs#L9)

- PNA preflight được hỗ trợ trong khi origin whitelist vẫn giữ nguyên.
  [`cors.rs:17`](../../src-tauri/src/interface/http_server/cors.rs#L17)

**Đóng gói không cần admin**

- Chỉ phát hành NSIS và cài trong tài khoản người dùng hiện tại.
  [`tauri.conf.json:26`](../../src-tauri/tauri.conf.json#L26)

- Dependency local TLS, certificate và Windows Service đã bị loại bỏ.
  [`Cargo.toml:81`](../../src-tauri/Cargo.toml#L81)

**Hợp đồng và kiểm thử**

- Webapp discovery dùng base URL HTTP loopback mới.
  [`webapp_integration.md:66`](../../docs/webapp_integration.md#L66)

- Test thực gọi ping qua HTTP mà không cần certificate.
  [`server.rs:75`](../../src-tauri/src/interface/http_server/server.rs#L75)
