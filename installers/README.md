# Installers

Install-time scripts + service unit files. Chạy trong context installer với quyền elevated.

## Windows (Wix MSI / NSIS)

1. Build binary chính (`sapo-printer.exe`) + agent (`sapo-printer-agent.exe`) vào `%INSTALL_DIR%`.
2. Copy `windows/register-service.ps1` + `windows/unregister-service.ps1` vào `%INSTALL_DIR%`.
3. Custom action lúc install:
   ```
   powershell.exe -ExecutionPolicy Bypass -File "%INSTALL_DIR%\register-service.ps1"
   ```
4. Custom action lúc uninstall:
   ```
   powershell.exe -ExecutionPolicy Bypass -File "%INSTALL_DIR%\unregister-service.ps1"
   ```

Script tự chạy `sapo-printer-agent --install-ca` để sinh cert + install vào `LocalMachine\Root`, sau đó `sc create` service.

Wix fragment: `windows/wix-fragment.wxs` — include vào Tauri Wix builder qua `tauri.conf.json > bundle > windows > wix > fragmentPaths`.

## macOS (pkg)

1. Bundle binary vào `/Applications/Sapo Printer.app`.
2. `postinstall`:
   - Copy `com.sapo.printer.agent.plist` → `/Library/LaunchDaemons/`.
   - Chạy `sapo-printer-agent --install-ca`.
   - `launchctl load /Library/LaunchDaemons/com.sapo.printer.agent.plist`.
3. `preremove`:
   - `launchctl unload ...`.
   - Chạy `sapo-printer-agent --uninstall-ca`.
   - Xóa plist.

## Linux (deb / rpm)

1. Binary → `/usr/bin/sapo-printer` + `/usr/bin/sapo-printer-agent`.
2. `postinst`:
   - Copy `sapo-printer-agent.service` → `/etc/systemd/system/`.
   - `getent group sapo-printer || groupadd -r sapo-printer`.
   - Add current user to group: `usermod -a -G sapo-printer $SUDO_USER`.
   - `sapo-printer-agent --install-ca`.
   - `systemctl daemon-reload && systemctl enable --now sapo-printer-agent.service`.
3. `prerm`:
   - `systemctl stop sapo-printer-agent.service`.
   - `systemctl disable sapo-printer-agent.service`.
   - `sapo-printer-agent --uninstall-ca`.
   - Xóa service file.

## Verify

Sau install, chạy:
```
sapo-printer-agent --check
```
Trả `Ok` → cert healthy.

```
curl -k https://local.mysapo.net:18901/api/v1/ping
```
Trả JSON với `status: ok`.
