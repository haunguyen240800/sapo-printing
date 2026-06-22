---
title: 'Unify runtime configuration and temporary storage paths'
type: 'bugfix'
created: '2026-08-24'
status: 'done'
baseline_commit: '3c8944f83031d0bccb8b9ba4199f0b1592734294'
context:
  - '{project-root}/CLAUDE.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Runtime data hiện bị tách giữa `%APPDATA%/sapo-printer-pro-max` và hai đường dẫn hard-code `~/.sapo-printer`: print config vẫn ghi vào folder cũ, còn downloader tạo PDF ngoài temp root nên `FilesystemTempFileManager` có thể từ chối file vừa tải. Người dùng cũng yêu cầu audit toàn bộ cấu hình để xác nhận không còn phần đặt tên/path thiếu đồng nhất.

**Approach:** Dùng `AppDirs` làm nguồn duy nhất cho SQLite, logs, agent metadata, print config và temporary downloads; inject path vào các infrastructure adapter thay vì tự đọc HOME. Xóa dependency không còn dùng, thêm regression tests và cập nhật tài liệu runtime.

## Boundaries & Constraints

**Always:** Lưu `print-config.json` và file tải tạm bên dưới OS data root `sapo-printer-pro-max`; Tauri save/get command và `ConfigPort` phải dùng cùng một path; downloader output phải được `FilesystemTempFileManager` chấp nhận; mọi path phải được inject/test bằng temp directory, không phụ thuộc HOME của test runner; giữ nguyên các thay đổi version 1.0.9 chưa commit của người dùng.

**Ask First:** Thay đổi hoặc xóa dữ liệu đang tồn tại ngoài workspace; thêm migration tự động từ `.sapo-printer`; đổi bundle/update identity.

**Never:** Đổi binary `sapo-printer`, crate/tracing namespace `sapo_printer`, bundle/keychain identifier `com.sapo.printer`, localStorage key `sapo-printer.pending-update-version`, OS scratch directories hoặc tài liệu planning TLS cũ chỉ vì chuỗi tên khác slug runtime.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|---------------|----------------------------|----------------|
| Save/load config | Injected `<data_dir>/print-config.json` | Tạo parent khi cần, round-trip đúng JSON tại canonical root | Trả lỗi có path context; không fallback HOME |
| Download PDF | Injected `<data_dir>/temp` | `.tmp` và `.pdf` nằm trong cùng temp root, wrap thành công | Xóa `.tmp` khi HTTP/validation/rename lỗi |
| Chưa có config | Canonical config file không tồn tại | Provider trả `Ok(None)` | Không tạo folder legacy |

</frozen-after-approval>

## Code Map

- `src-tauri/src/bootstrap/dirs.rs` — nguồn canonical cho data/log/temp/DB/config paths.
- `src-tauri/src/lib.rs` — khởi tạo directories và truyền vào composition root.
- `src-tauri/src/bootstrap/app_state.rs` — inject config path/temp root vào adapters và managed state.
- `src-tauri/src/infrastructure/configs/app/app_print_config.rs` — JSON file I/O hiện đang hard-code HOME.
- `src-tauri/src/infrastructure/configs/app/json_file_config_provider.rs` — `ConfigPort` adapter cần giữ injected path.
- `src-tauri/src/infrastructure/integrations/network/reqwest_downloader.rs` — downloader cần giữ injected temp root.
- `src-tauri/src/interface/tauri/commands/printer_command.rs` — desktop save/get config phải dùng canonical path từ state.
- `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` — loại `home` crate nếu không còn consumer.
- `CLAUDE.md` — mô tả một runtime data root và các technical identity được giữ có chủ đích.

## Tasks & Acceptance

**Execution:**
- [x] `bootstrap/dirs.rs`, `lib.rs`, `bootstrap/app_state.rs` — thêm/wire canonical print-config path và temp root qua composition root.
- [x] `configs/app/*.rs`, `printer_command.rs` — chuyển JSON config I/O/provider/commands sang injected path; thêm filesystem tests.
- [x] `reqwest_downloader.rs`, `temp_file.rs` — inject temp directory và test output nằm trong root được RAII manager chấp nhận.
- [x] `Cargo.toml`, `Cargo.lock`, `CLAUDE.md` — xóa dependency HOME không dùng và đồng bộ hướng dẫn/audit identity.

**Acceptance Criteria:**
- Given source runtime sau thay đổi, when quét `.sapo-printer`, `USERPROFILE`, `HOME` và `home::home_dir`, then không còn storage path runtime nào tự dựng từ home directory.
- Given một temp data root, when save rồi load print config, then file duy nhất được tạo tại `<data_root>/print-config.json` và nội dung round-trip đúng.
- Given downloader và temp manager dùng cùng injected directory, when một PDF hợp lệ được tải, then path được wrap chấp nhận và cleanup theo RAII.
- Given audit tên cấu hình, when đối chiếu manifest/runtime, then canonical product/data slug nhất quán và các identifier kỹ thuật được giữ đều có rationale trong `CLAUDE.md`.

## Spec Change Log

## Design Notes

`AppDirs` thuộc bootstrap và chỉ composition root biết OS-specific base directory. Infrastructure adapters nhận `PathBuf` cụ thể qua constructor; chúng không phụ thuộc Tauri hoặc tự giải quyết HOME. Không tự di chuyển/xóa `.sapo-printer` vì thao tác dữ liệu cần quyết định riêng.

## Verification

**Commands:**
- `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::configs::app` — config serialization và path I/O pass.
- `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::integrations::network` — downloader path/cleanup tests pass.
- `cargo test --manifest-path src-tauri/Cargo.toml infrastructure::temp_file` — containment và RAII tests pass.
- `cargo check --manifest-path src-tauri/Cargo.toml` — backend compile thành công.
- `pnpm run build:web` — frontend typecheck/build thành công.
- `rg -n "\\.sapo-printer|USERPROFILE|home::home_dir" src-tauri/src` — không còn legacy runtime path resolver.

## Suggested Review Order

**Composition root và canonical paths**

- Bắt đầu từ dependency graph để thấy cả config và temp dùng chung `AppDirs`.
  [`app_state.rs:49`](../../src-tauri/src/bootstrap/app_state.rs#L49)

- Canonical print-config path được tạo cạnh database, logs và temp.
  [`dirs.rs:35`](../../src-tauri/src/bootstrap/dirs.rs#L35)

- Tauri setup truyền path duy nhất vào managed application state.
  [`lib.rs:65`](../../src-tauri/src/lib.rs#L65)

**Config JSON path injection**

- File I/O nhận path cụ thể, tạo parent khi save và không fallback HOME.
  [`app_print_config.rs:45`](../../src-tauri/src/infrastructure/configs/app/app_print_config.rs#L45)

- Application port adapter giữ injected path thay vì tự resolve môi trường.
  [`json_file_config_provider.rs:6`](../../src-tauri/src/infrastructure/configs/app/json_file_config_provider.rs#L6)

- Desktop commands dùng cùng path trong `AppContextState`.
  [`printer_command.rs:72`](../../src-tauri/src/interface/tauri/commands/printer_command.rs#L72)

**Downloader và temp containment**

- Downloader tạo `.tmp` và `.pdf` trực tiếp dưới injected temp root.
  [`reqwest_downloader.rs:19`](../../src-tauri/src/infrastructure/integrations/network/reqwest_downloader.rs#L19)

- Regression test xác nhận output được RAII containment chấp nhận và cleanup.
  [`reqwest_downloader.rs:306`](../../src-tauri/src/infrastructure/integrations/network/reqwest_downloader.rs#L306)

**Documentation và identity boundaries**

- Runtime storage root và path injection được ghi lại theo source hiện tại.
  [`CLAUDE.md:118`](../../CLAUDE.md#L118)

- Binary, crate, bundle, keychain và localStorage identities được giữ có chủ đích.
  [`CLAUDE.md:123`](../../CLAUDE.md#L123)
