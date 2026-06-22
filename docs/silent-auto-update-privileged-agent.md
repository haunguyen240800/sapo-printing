# Thiết kế: Silent Auto-Update qua Privileged Agent Service

> Trạng thái: **Đã chốt quyết định (§11) — chờ user xác nhận "implement" thì mới code**
> Cập nhật 2026-08-18: chốt fallback mềm, cài ngay (multi-user), Windows Service, gộp renewal,
> forced update + fail-open + mọi version đều bắt buộc.
> Bối cảnh: Môi trường SAPO retail có nhiều user login chung một máy → CA phải trust
> toàn máy (`LocalMachine\Root`), app cài perMachine (`Program Files`). Việc này khiến
> auto-update hiện tại luôn bật UAC. Tài liệu này phác mô hình để update im lặng.

## 1. Mục tiêu & ràng buộc

| | |
|---|---|
| **Mục tiêu** | Update im lặng, không UAC, sau lần cài đầu |
| **Ràng buộc cứng** | CA trust toàn máy (`LocalMachine\Root`); app cài perMachine (`Program Files`) — không đổi |
| **Chấp nhận** | 1 lần UAC ở cài đặt đầu tiên (không tránh được, và bình thường) |
| **Nguyên tắc bảo mật** | IPC chỉ gửi *tín hiệu*; service tự đọc `latest.json` từ endpoint cố định + verify minisign trước khi chạy installer |

## 2. Vì sao không thể bỏ UAC bằng cách đổi install mode

Có 2 nguồn cần quyền admin khi update:

| Nguồn | Vì sao cần admin |
|---|---|
| (A) Ghi đè file app ở `Program Files` | perMachine install |
| (B) Cài cert vào `LocalMachine\Root` (`certutil -addstore Root`) | trust toàn máy |

Vì trust toàn máy là bắt buộc, không thể chuyển sang per-user để né (A)+(B).
Cách duy nhất để update im lặng: có sẵn một tiến trình **đã elevated (SYSTEM)** làm thay.
Dự án đã có `cert-manager` với chế độ daemon + IPC → mở rộng nó thành "privileged agent".

## 3. Kiến trúc trước / sau

**Hiện tại (UAC mỗi update):**
```
App (user) -> tauri-plugin-updater.download_and_install()
           -> chạy setup.exe trong user-context -> UAC prompt -> cài
```

**Đề xuất (im lặng):**
```
App (user) -> phát hiện update (giữ tauri-plugin-updater cho phần CHECK)
           -> IPC "request_update" -> [Named Pipe]
                                          v
cert-manager Service (LocalSystem, chạy sẵn từ lúc cài)
   -> đọc latest.json từ endpoint cố định
   -> tải setup.exe -> verify minisign (pubkey nhúng sẵn)
   -> chạy setup.exe /S  (SYSTEM -> KHÔNG UAC)
   -> cài xong -> relaunch app trong session user (CreateProcessAsUser)
```

## 4. Quyết định thiết kế then chốt

1. **Giữ `tauri-plugin-updater` cho phần CHECK** (background loop 24h + nút "Kiểm tra").
   Chỉ **thay phần INSTALL** bằng delegate sang service. Ít rủi ro, tái dùng cái đang chạy tốt.
2. **Service tự đọc `latest.json`, KHÔNG nhận URL/signature qua IPC.** Chống user thường
   lừa service SYSTEM cài file lạ. IPC tối đa chỉ mang `expected_version` để đối chiếu.
3. **Biến `cert-manager` daemon thành Windows Service thật** (thêm SCM integration — đúng
   "Sprint 7" mà code đã dự trù), thay vì scheduled task. Robust hơn về start/stop/restart.
4. **Relaunch app bằng `CreateProcessAsUser`** với token của active console session
   (giải quyết session-0 isolation của service).

## 5. Luồng update chi tiết (sequence)

```
1. Background loop (bootstrap/updater.rs) hoặc user bấm "Kiểm tra phiên bản"
2. tauri-plugin-updater.check() -> có version mới -> emit "update-available"
3. User bấm "Tải và cài đặt"
4. App -> IPC { cmd: "request_update", expected_version: "1.1.0" }
5. Service:
   a. Đọc latest.json từ endpoint cố định (hard-coded repo)
   b. Đối chiếu version == expected_version && > version đang cài (chống downgrade)
   c. Tải setup.exe -> %ProgramData%\SapoPrinter\update\
   d. Verify minisign signature (pubkey nhúng) — FAIL thì abort + xóa file
   e. (optional) Trả IPC progress -> app hiển thị %
   f. IPC "prepare_restart" -> app lưu state, exit
   g. Chạy setup.exe /S (SYSTEM) -> ghi Program Files + cert hook (no UAC)
   h. CreateProcessAsUser -> relaunch app trong session active
6. App khởi động lại ở version mới
```

## 6. Danh sách thay đổi từng file

### A. Installer — đăng ký service

| File | Thay đổi |
|---|---|
| `src-tauri/tauri.conf.json` | Thêm `sapo-printer-cert-manager.exe` vào `bundle.resources` để copy vào INSTDIR |
| `installers/windows/nsis-hooks.nsh` | `NSIS_HOOK_POSTINSTALL`: sau khi cài cert, `sc create SapoPrinterAgent binPath= "$INSTDIR\sapo-printer-cert-manager.exe" start= auto obj= LocalSystem` + `sc start`. Phân biệt install mới vs update (chỉ tạo khi chưa có) |
| `installers/windows/nsis-hooks.nsh` | Thêm `NSIS_HOOK_PREUNINSTALL`: `sc stop` + `sc delete SapoPrinterAgent` |
| `installers/windows/wix-fragment.wxs` | Parity cho MSI: `<ServiceInstall>` + `<ServiceControl>` (Start on install, Stop+Remove on uninstall) |
| `installers/windows/install-ca-cert.ps1` | Giữ nguyên (service tự lo cert; hook vẫn chạy được dưới SYSTEM) |

### B. Service — SCM integration + update executor

| File | Thay đổi |
|---|---|
| `src-tauri/Cargo.toml` | Deps cho bin cert-manager: `windows-service` (SCM), `minisign-verify` (verify chữ ký); đảm bảo `reqwest` có sẵn |
| `src-tauri/src/bin/sapo_printer_cert_manager.rs` | Bọc `main` bằng service dispatcher (`windows-service`): control handler (Stop/Shutdown), chạy daemon hiện có bên trong. Giữ CLI mode (`--install-ca`...) cho installer |
| `src-tauri/src/infrastructure/platform/updater/service_updater.rs` | **File mới**: `read_latest_json(endpoint)`, `download(url)->path`, `verify_signature(path, sig, pubkey)`, `run_installer_silent(path)`, `relaunch_in_user_session(exe)` (CreateProcessAsUser) |
| `src-tauri/src/infrastructure/platform/tls/ipc.rs` | Thêm `IpcRequest::RequestUpdate { expected_version }` và `IpcRequest::PrepareRestart`; field progress vào `IpcResponse` nếu stream |
| `sapo_printer_cert_manager.rs::handle_request` | Xử lý `RequestUpdate` theo luồng mục 5. Hard-code endpoint + pubkey (không nhận từ client) |
| `sapo_printer_cert_manager.rs::windows_ipc` | **Siết DACL** named pipe: chỉ Authenticated Users connect (security descriptor SDDL) |

### C. App backend (Tauri) — đổi install path

| File | Thay đổi |
|---|---|
| `src-tauri/src/interface/tauri/commands/update_command.rs` | `install_update`: thay `download_and_install_update(&app)` bằng gửi IPC `RequestUpdate`. Giữ `install_guard`. `check_for_updates` giữ nguyên |
| `src-tauri/src/infrastructure/platform/updater/update_checker.rs` | Giữ `check_for_updates`. `download_and_install_update` giữ làm fallback hoặc bỏ |
| `src-tauri/src/infrastructure/platform/updater/agent_client.rs` | **File mới**: client connect named pipe, gửi `RequestUpdate`, đọc progress/response |
| `src-tauri/src/bootstrap/updater.rs` | Giữ nguyên logic check + emit `update-available` |
| `src-tauri/src/bootstrap/app_state.rs` | Nếu cần, inject agent IPC client vào `AppContextState` |

### D. Frontend — gần như không đổi

| File | Thay đổi |
|---|---|
| `src/services/update-service.ts` | Giữ API (`installUpdate` vẫn `invoke("install_update")`). Thêm listen progress event nếu service stream |
| `src/pages/printer/hooks/useAppUpdate.ts` | Nếu có progress thật -> cập nhật `installing.progress` thay vì 0/100 |
| `src/pages/printer/components/AppInfoModal.tsx` | Không đổi (đã fix "unknown") |

### E. Build / release

| File | Thay đổi |
|---|---|
| `scripts/release.sh` / `release.ps1` | Không đổi cơ chế; đảm bảo field `latest.json` khớp cái service parse |
| `tauri.conf.json` bundle | Xác nhận `cert-manager.exe` build ra và nằm trong installer |

## 7. Yêu cầu bảo mật (bắt buộc)

1. **Verify minisign trước khi execute** — không bao giờ chạy file chưa verify. Pubkey nhúng
   trong binary (const), khớp `tauri.conf.json`.
2. **Endpoint cố định** — service hard-code repo/host, không nhận URL từ IPC.
3. **DACL named pipe** — chỉ user đăng nhập hợp lệ connect; IPC chỉ cho "trigger".
4. **Chống downgrade** — chỉ cài khi version mới > version đang chạy.
5. **Xóa file tải nếu verify fail**; audit log mọi lần update.

## 8. Edge cases & rủi ro

| Vấn đề | Xử lý |
|---|---|
| Service chưa chạy / crash khi update | **Fallback mềm (đã chốt)**: app tự dùng tauri-updater cũ `download_and_install()` (có UAC). Không chặn update |
| Relaunch app từ session 0 (service) | `WTSGetActiveConsoleSessionId` + `WTSQueryUserToken` + `CreateProcessAsUser`. Không có session active -> cài xong, không relaunch |
| App đang chạy khi ghi đè file | Service yêu cầu app exit trước (IPC PrepareRestart), chờ process thoát rồi chạy installer |
| Nhiều user login cùng lúc | Service SYSTEM cài ngay (không chờ session). Relaunch cho active console session; các session khác lên version mới ở lần mở app kế tiếp |
| SCM integration sai -> service không start | Test kỹ; giữ CLI mode độc lập với service mode |
| User thường lạm dụng named pipe | Chặn bằng "chỉ trigger + verify signature" + DACL |

## 9. Kế hoạch test

- **Unit**: parse latest.json; verify signature (valid/invalid/tampered); version compare; downgrade reject.
- **Integration**: service nhận IPC RequestUpdate -> tải mock (mockito) -> verify -> (mock) run.
- **Manual E2E**: cài 1.0.0 (perMachine, service registered) -> release 1.1.0 -> bấm cài ->
  **xác nhận KHÔNG có UAC** -> app relaunch ở 1.1.0 -> cert vẫn trusted, HTTPS local OK.
- **Multi-user**: login 2 user, update từ 1 session, kiểm tra user kia sau relaunch.
- **Uninstall**: service bị stop + delete sạch.

## 10. Thứ tự triển khai đề xuất

1. SCM integration cho cert-manager (chạy được như service) + đăng ký service trong installer.
   **Test service sống độc lập trước.**
2. Wire renewal IPC thật (vá lỗ hổng renewal đang chết) — làm luôn vì service đã chạy.
3. Thêm IPC `RequestUpdate` + `service_updater` (tải + verify + chạy /S).
4. Relaunch in user session.
5. Đổi `install_update` command sang IPC path; giữ fallback.
6. Siết DACL pipe + audit log.
7. Frontend progress (optional).

## 11. Quyết định đã chốt (2026-08-18)

| # | Vấn đề | Quyết định |
|---|---|---|
| 1 | Fallback khi service lỗi | **Rơi về update-có-UAC cũ** (graceful degradation). Service chết/không có → app tự `download_and_install()` (có UAC). Không bao giờ chặn update. |
| 2 | Thời điểm update multi-user | **Cài ngay khi phát hiện** (service SYSTEM, không phụ thuộc session). App chỉ prompt "restart để áp dụng". Không chờ "1 session active". |
| 3 | SCM vs Scheduled Task | **Windows Service (SCM)** — đồng ý. |
| 4 | Renewal IPC | **Gộp vào đợt này** — service đã chạy thì wire luôn RenewNow/GetStatus/RotateCa, kèm DACL siết quyền lệnh đặc quyền. |
| 5 | Forced update | **Có — bắt buộc.** Có version mới thì khóa app tới khi update xong (xem §13). |
| 6 | Mất mạng / không check được | **Fail-open** — không check được thì vẫn cho dùng; chỉ khóa khi *xác nhận* có version mới. |
| 7 | Phạm vi bắt buộc | **Mọi version mới đều bắt buộc.** (Chừa sẵn field `critical`/`minimum_version` trong `latest.json` cho tương lai, nhưng trước mắt mọi bản đều force.) |

## 13. Forced update (bắt buộc cập nhật)

**Yêu cầu:** Có version mới thì app phải update lên mới nhất mới dùng được.

### Luồng gate khi khởi động
```
1. App start -> check version (blocking, trước khi cho vào UI chính)
2a. Đang latest            -> vào app bình thường
2b. Check FAIL (mất mạng)  -> FAIL-OPEN: vào app bình thường (không khóa)
2c. Có version mới         -> hiện MODAL CHẶN không tắt được:
      "Có bản cập nhật bắt buộc" -> tự trigger update (service silent / fallback UAC)
      -> update xong -> restart -> lên latest -> dùng được
3. Phát hiện version mới giữa phiên (loop 24h): cũng bật modal chặn -> buộc update
```

### Quy tắc
- Modal forced update **không có nút "Để sau"**, không đóng được.
- **Fail-open**: chỉ khóa khi backend *xác nhận* `check()` trả về version mới. Lỗi mạng / lỗi GitHub → coi như chưa biết → cho dùng.
- **Mọi version mới đều force** (so sánh `latest > current`).
- **Update fail** (mạng rớt / service lỗi / user từ chối UAC ở fallback): modal cho **"Thử lại"**; sau N lần fail cho **"Thoát app"** (không cho dùng phiên cũ, nhưng không treo).

### Thay đổi bổ sung cho forced update

| File | Thay đổi |
|---|---|
| `src/pages/printer/hooks/useAppUpdate.ts` | Thêm state `forced: boolean`. Khi check thấy version mới → set forced=true |
| `src/pages/printer/components/` | **Component mới** `ForcedUpdateModal.tsx`: overlay full-screen, không dismiss, nút "Thử lại"/"Thoát" khi fail |
| `src/App.tsx` (hoặc router gốc) | Gate: nếu `forced` → render `ForcedUpdateModal` đè lên toàn app, chặn tương tác UI chính |
| `src/services/update-service.ts` | Thêm `quitApp()` (invoke lệnh Tauri thoát app) cho nhánh "Thoát" sau khi fail |
| `src-tauri/.../update_command.rs` | Thêm command `quit_app` (app.exit). Startup check trả rõ trạng thái "có update mới" để frontend gate |
| `src-tauri/src/bootstrap/updater.rs` | Check startup phân biệt 3 trạng thái: latest / có-update / check-fail (fail-open) |

### Tương tác với privileged service
- Forced update xảy ra **thường xuyên** → nếu mỗi lần đều UAC thì cực tệ. → Service silent là **must-have** để forced update mượt.
- Fallback UAC vẫn giữ (service chết) — user vẫn update được, chỉ là có prompt.

## 12b. Câu (đã trả lời) — giữ để tra cứu
Các câu ở §11 bản trước đã được chốt hết trong bảng §11 hiện tại.

## 12. Phụ lục — hiện trạng codebase liên quan

- `cert-manager` là bin thứ 2 (`Cargo.toml [[bin]] name = "sapo-printer-cert-manager"`), có
  2 mode: one-shot (`--install-ca/--renew/--check`) và daemon (IPC server + renewal loop 24h).
- **Hiện chỉ dùng one-shot** qua NSIS hook (`nsis-hooks.nsh`) + WiX custom action
  (`wix-fragment.wxs`). Daemon/service **chưa được đăng ký** (comment "Sprint 7 SCM integration").
- Hệ quả: IPC renewal hiện **không có ai lắng nghe** -> renewal tự động chưa chạy (lỗ hổng sẵn).
- CA lưu ở `%ProgramData%\SapoPrinter` (`cert_dir.rs`) — ngoài install dir, tồn tại qua update.
  `CertGenerator::load_or_generate` reuse CA cũ -> update không tái tạo CA.
- IPC: named pipe `\\.\pipe\sapo-printer-agent` (`ipc.rs`), protocol JSON newline-delimited,
  hiện có `RenewNow/GetStatus/RotateCa/Ping`; `windows_ipc` **chưa set DACL**.
- Updater hiện tại: `tauri-plugin-updater`, endpoint GitHub `latest.json`, pubkey minisign
  nhúng trong `tauri.conf.json`, installMode `passive`.
