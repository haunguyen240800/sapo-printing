# Code Signing & Auto-Update Release Guide

Tài liệu hướng dẫn ký code và phát hành bản cập nhật cho Sapo Printer.

## 1. Key Generation

Tạo cặp khóa ký (signing keypair) bằng Tauri CLI:

```bash
cargo tauri signer generate -w ~/.tauri/sapo-printer.key
```

Lệnh này tạo 2 file:
- `~/.tauri/sapo-printer.key` — **Private key** (giữ bí mật, không commit lên git)
- `~/.tauri/sapo-printer.key.pub` — **Public key** (nhúng vào `tauri.conf.json`)

> **Lưu ý:** Thêm `~/.tauri/sapo-printer.key` vào `.gitignore`. Không bao giờ commit private key.

## 2. Public Key Configuration

Đọc nội dung file `.pub` và paste vào `src-tauri/tauri.conf.json`:

```json
{
  "plugins": {
    "updater": {
      "pubkey": "<NỘI_DUNG_FILE_.PUB>",
      "endpoints": [
        "https://github.com/<org>/<repo>/releases/latest/download/latest.json"
      ]
    }
  }
}
```

> **Quan trọng:** `pubkey` phải là nội dung raw của file `.pub` (chuỗi PEM), không phải đường dẫn file.

## 3. Build-Time Environment Variables

Khi build release, cần set biến môi trường để Tauri ký artifacts:

### Windows (PowerShell)

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY="C:\Users\<user>\.tauri\sapo-printer.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
cargo tauri build
```

### Windows (CMD)

```cmd
set TAURI_SIGNING_PRIVATE_KEY=C:\Users\<user>\.tauri\sapo-printer.key
set TAURI_SIGNING_PRIVATE_KEY_PASSWORD=
cargo tauri build
```

### Linux / macOS

```bash
export TAURI_SIGNING_PRIVATE_KEY="$HOME/.tauri/sapo-printer.key"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
cargo tauri build
```

> Nếu key không có password, để `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` trống.

## 4. Release Workflow

### Bước 1: Build

```bash
cargo tauri build
```

Khi `createUpdaterArtifacts: true` được bật, Tauri bundler tự động tạo:
- Bundle files (`.msi`, `.exe`, `.AppImage`, etc.)
- `.sig` signature files cho mỗi bundle

### Bước 2: Tạo `latest.json` manifest

File `latest.json` chứa thông tin version và chữ ký cho mỗi platform:

```json
{
  "version": "0.2.0",
  "notes": "Sửa lỗi và cải thiện hiệu suất",
  "pub_date": "2026-06-25T00:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "NỘI_DUNG_FILE_.SIG",
      "url": "https://github.com/org/repo/releases/download/v0.2.0/sapo-printer_0.2.0_x64_en-US.msi.zip"
    },
    "linux-x86_64": {
      "signature": "NỘI_DUNG_FILE_.SIG",
      "url": "https://github.com/org/repo/releases/download/v0.2.0/sapo-printer_0.2.0_amd64.AppImage.tar.gz"
    },
    "darwin-x86_64": {
      "signature": "NỘI_DUNG_FILE_.SIG",
      "url": "https://github.com/org/repo/releases/download/v0.2.0/sapo-printer_0.2.0_x64.app.tar.gz"
    },
    "darwin-aarch64": {
      "signature": "NỘI_DUNG_FILE_.SIG",
      "url": "https://github.com/org/repo/releases/download/v0.2.0/sapo-printer_0.2.0_aarch64.app.tar.gz"
    }
  }
}
```

> **Quan trọng:** Field `signature` phải là nội dung raw của file `.sig`, không phải URL hay path.

### Bước 3: Upload lên GitHub Releases

1. Tạo release mới trên GitHub với tag `v0.2.0`
2. Upload tất cả bundle files + `.sig` files
3. Upload `latest.json` (phải downloadable tại URL endpoint)

### Bước 4: Verify

App sẽ tự động check update:
- **On startup:** Check ngay khi app khởi động
- **Every 24 hours:** Check định kỳ mỗi 24 giờ

Khi có update mới, event `"update-available"` được emit để frontend hiển thị popup.

## 5. Platform-Specific Code Signing (Optional)

Phần này dành cho enterprise distribution ngoài Tauri updater.

### Windows — EV Certificate

Dùng EV code signing certificate + `signtool`:

```bash
signtool sign /fd sha256 /tr http://timestamp.digicert.com /td sha256 \
  /f certificate.pfx /p PASSWORD \
  target/release/sapo-printer.exe
```

Required cho:
- Phân phối qua Windows Store
- Tránh Windows SmartScreen warnings
- Enterprise environments với code signing policies

### macOS — Apple Developer ID + Notarization

```bash
# Sign app bundle
codesign --deep --force --verify --verbose \
  --sign "Developer ID Application: Your Name (TEAM_ID)" \
  target/release/bundle/macos/sapo-printer.app

# Notarize with Apple
xcrun notarytool submit \
  target/release/bundle/macos/sapo-printer.app.zip \
  --apple-id "your@email.com" \
  --team-id "TEAM_ID" \
  --password "app-specific-password" \
  --wait

# Staple notarization ticket
xcrun stapler staple target/release/bundle/macos/sapo-printer.app
```

Required cho:
- Phân phối ngoài Mac App Store
- Tránh "unidentified developer" warnings trên macOS Gatekeeper

### Linux — GPG Signing (AppImage)

```bash
# Sign AppImage with GPG
gpg --armor --detach-sign \
  target/release/bundle/appimage/sapo-printer.appimage
```

Optional — chủ yếu dùng cho verification trong enterprise environments.

## 6. Endpoint URL Templates

Updater endpoint hỗ trợ template variables:

| Variable | Mô tả | Ví dụ |
|---|---|---|
| `{{target}}` | Platform target | `windows-x86_64`, `darwin-aarch64` |
| `{{arch}}` | CPU architecture | `x86_64`, `aarch64` |
| `{{current_version}}` | Version hiện tại | `0.1.0` |

### Static URL (GitHub Releases)

```
https://github.com/<org>/<repo>/releases/latest/download/latest.json
```

### Dynamic URL

```
https://github.com/<org>/<repo>/releases/download/v{{current_version}}/latest.json
```

## 7. Troubleshooting

| Vấn đề | Nguyên nhân | Giải pháp |
|---|---|---|
| "Updater init failed" | Plugin chưa được register | Kiểm tra `app.handle().plugin()` trong `.setup()` |
| "Update check failed" | Endpoint không reachable | Kiểm tra URL trong `tauri.conf.json` |
| "signature verification failed" | Pubkey không match | Đảm bảo pubkey trong config đúng với private key |
| App không check update | Thiếu `updater:default` capability | Thêm vào `capabilities/default.json` |
| Build lỗi `.sig` files | Thiếu signing key env vars | Set `TAURI_SIGNING_PRIVATE_KEY` |
