# Installers

Install-time scripts + service unit files. Chạy trong context installer với quyền elevated.

## Windows (Wix MSI / NSIS)

1. Build binary chính (`sapo-printer.exe`) + cert-manager (`sapo-printer-cert-manager.exe`) vào `%INSTALL_DIR%`.
2. Copy `windows/install-ca-cert.ps1` + `windows/uninstall-ca-cert.ps1` vào `%INSTALL_DIR%`.
3. Custom action lúc install:
   ```
   powershell.exe -ExecutionPolicy Bypass -File "%INSTALL_DIR%\install-ca-cert.ps1"
   ```
4. Custom action lúc uninstall:
   ```
   powershell.exe -ExecutionPolicy Bypass -File "%INSTALL_DIR%\uninstall-ca-cert.ps1"
   ```

Script tự chạy `sapo-printer-cert-manager --install-ca` để sinh CA + server cert và install CA vào `LocalMachine\Root`.

Cấu hình bundle:
- Wix (MSI): `windows/wix-fragment.wxs` — include qua `tauri.conf.json > bundle > windows > wix > fragmentPaths`.
- NSIS: `windows/nsis-hooks.nsh` — include qua `tauri.conf.json > bundle > windows > nsis > installerHooks`.

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
