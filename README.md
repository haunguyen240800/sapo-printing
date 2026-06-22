# Sapo Printer Pro Max

Ứng dụng in ấn desktop cho SAPO, xây dựng bằng [Tauri 2](https://tauri.app/) + Rust + TypeScript.

## Yêu cầu

- [Node.js](https://nodejs.org/) >= 18
- [pnpm](https://pnpm.io/) >= 8
- [Rust](https://www.rust-lang.org/) (stable toolchain)

## Cài đặt & chạy dev

```bash
pnpm install
pnpm tauri dev
```

## Build production

```bash
pnpm tauri build
```

Output installer (MSI / NSIS) sẽ nằm ở `src-tauri/target/release/bundle/`.

---

## Thư viện PDFium

Project sử dụng [pdfium-render](https://crates.io/crates/pdfium-render) (crate Rust) để render PDF thành bitmap trước khi in.

`pdfium-render` yêu cầu file native library **`pdfium.dll`** (Windows) tại runtime. File này **đã được commit sẵn** vào repo tại:

```
src-tauri/bin/pdfium.dll
```

Tauri tự động đóng gói `pdfium.dll` vào installer thông qua cấu hình `bundle.resources` trong [`tauri.conf.json`](src-tauri/tauri.conf.json):

```json
"resources": {
  "bin/pdfium.dll": "./"
}
```

### Cập nhật pdfium.dll

Nếu cần upgrade phiên bản PDFium, tải pre-built binary từ:

- **Nguồn chính thức:** https://github.com/bblanchon/pdfium-binaries/releases

Giải nén và thay thế `src-tauri/bin/pdfium.dll` bằng file mới.

> **Lưu ý:** File `pdfium-win-x64.tgz` (nguồn gốc của `pdfium.dll` hiện tại) đã bị xóa khỏi repo vì không cần thiết trong quá trình build — chỉ cần `bin/pdfium.dll`.
