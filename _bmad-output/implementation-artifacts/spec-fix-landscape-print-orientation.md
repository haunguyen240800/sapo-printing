---
title: 'Sửa in Landscape vẫn ra Portrait'
type: 'bugfix'
created: '2026-09-04'
status: 'done'
baseline_commit: '72ce655bf23fbd2bbad63679bd2b0ba7f2e263cf'
context:
  - '{project-root}/CLAUDE.md'
  - '{project-root}/_bmad-output/implementation-artifacts/investigations/landscape-prints-portrait-investigation.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** `orientation = Landscape` được lưu đúng vào print job nhưng production `BitmapRenderStrategy` không dùng giá trị này. Graphics backend không nhận orientation và Windows DEVMODE giữ mặc định driver hoặc ép Portrait khi fallback, nên bản in vẫn dọc.

**Approach:** Dùng kiểu orientation nội bộ, truyền nó qua renderer/backend và áp dụng vào DEVMODE Windows. Giữ kích thước media, page geometry và `rotate` là các khái niệm độc lập.

## Boundaries & Constraints

**Always:** Production tiếp tục dùng `BitmapRenderStrategy`; giữ full DEVMODE cùng private driver data; đặt `DM_ORIENTATION` và giá trị tương ứng trước `CreateDCW`; cho driver merge/validate cấu hình bằng `DM_IN_BUFFER | DM_OUT_BUFFER`; cập nhật mọi implementation của `GraphicsBackend`; bảo toàn thay đổi chưa commit.

**Ask First:** Thay đổi JSON/API công khai, semantics của `paper_size`, render-strategy selection hoặc cơ chế một spooler document cho mỗi trang.

**Never:** Không map Landscape thành `rotate = 90`; không bật `native_pdf_render`; không đọc lại config trong worker; không thêm persistence; không sửa UI nếu backend tiêu thụ được chuỗi hiện tại.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|---------------|----------------------------|----------------|
| Landscape | Khổ không vuông; `Landscape` | Backend nhận Landscape; Windows dùng `DMORIENT_LANDSCAPE`; page ngang | Lỗi driver/HDC trả error, không panic |
| Portrait | `Portrait` | Explicit Portrait; giữ scale hiện tại | N/A |
| Chuỗi cũ | `LANDSCAPE`/`landscape` | Nhận diện Landscape | N/A |
| Chuỗi lạ | Orientation không hợp lệ | Fallback Portrait | Không panic |
| Query thành công | Full DEVMODE có private data | Patch field cần thiết rồi driver merge/validate | Không dùng buffer chưa validate |
| Query thất bại | Không lấy được full DEVMODE | Fallback phản ánh orientation yêu cầu | Ghi warning |
| DC không có size | `(0, 0)` | Fallback dùng dimensions hiệu dụng | Mỗi chiều ≥ 1 px |

</frozen-after-approval>

## Code Map

- `src-tauri/src/infrastructure/platform/printer_api/backend.rs` — graphics contract và orientation type.
- `src-tauri/src/infrastructure/integrations/pdf_engine/bitmap_strategy.rs` — propagation và fallback geometry.
- `src-tauri/src/infrastructure/platform/printer_api/windows.rs` — DEVMODE/HDC.
- `src-tauri/src/infrastructure/platform/printer_api/macos.rs` — CUPS media geometry và trait implementation.
- `src-tauri/src/infrastructure/platform/printer_api/linux.rs` — CUPS media geometry và trait implementation.
- `src-tauri/src/domain/print_job/value_objects/paper_size.rs` — kích thước media portrait, không đổi semantics.

## Tasks & Acceptance

**Execution:**
- [x] `printer_api/backend.rs` — thêm orientation enum: parse không phân biệt hoa/thường, fallback Portrait, helper tính effective dimensions; mở rộng `begin_document`.
- [x] `bitmap_strategy.rs` — derive orientation một lần, truyền xuống backend và dùng effective dimensions khi DC không báo size; không liên kết với `rotate`.
- [x] `printer_api/windows.rs` — patch orientation ở driver/fallback paths, merge full DEVMODE qua driver trước `CreateDCW`, tách helper thuần để test.
- [x] `printer_api/macos.rs`, `printer_api/linux.rs` — tương thích contract và dùng effective dimensions cho page pixels/CUPS media.
- [x] Tests cạnh module — bao phủ parse/dimensions, Windows DEVMODE fields, Portrait regression và propagation.

**Acceptance Criteria:**
- Given Landscape với khổ không vuông, when bitmap pipeline bắt đầu document, then backend nhận Landscape và geometry được đổi chiều đúng một lần.
- Given Windows trả full DEVMODE, when tạo HDC cho Landscape, then DEVMODE đã merge có `DM_ORIENTATION`/`DMORIENT_LANDSCAPE` và còn private data.
- Given query driver thất bại, when tạo fallback DEVMODE, then orientation theo request thay vì luôn Portrait.
- Given Portrait hoặc chuỗi lạ, when in, then giữ hành vi Portrait và không panic.
- Given `rotate = 90`, when render ở bất kỳ orientation nào, then chỉ nội dung bị xoay theo `rotate`.

## Spec Change Log

## Design Notes

`paper_size.dimensions_mm()` vẫn là kích thước media theo portrait orientation. Orientation điều khiển page/DC coordinate system; `rotate` chỉ xoay nội dung. Sau khi sửa DEVMODE fields, gọi lại `DocumentPropertiesW` với input/output buffer để driver merge/validate trước `CreateDCW`.

## Verification

**Commands:**
- `cargo fmt --check --manifest-path src-tauri/Cargo.toml`
- `cargo check --manifest-path src-tauri/Cargo.toml`
- `cargo test --manifest-path src-tauri/Cargo.toml`

**Manual checks:**
- Trên printer Windows default Portrait, in khổ không vuông ở Portrait/Landscape; xác nhận chiều giấy, scale, `rotate` và một spooler document mỗi PDF page.

## Suggested Review Order

**Orientation propagation**

- Derives physical/effective geometry once and keeps page orientation independent from content rotation.
  [`bitmap_strategy.rs:9`](../../src-tauri/src/infrastructure/integrations/pdf_engine/bitmap_strategy.rs#L9)

- Defines the shared orientation contract consumed by every graphics backend.
  [`backend.rs:8`](../../src-tauri/src/infrastructure/platform/printer_api/backend.rs#L8)

**Windows driver boundary**

- Merges explicit orientation into full driver DEVMODE and rejects driver normalization.
  [`windows.rs:106`](../../src-tauri/src/infrastructure/platform/printer_api/windows.rs#L106)

- Creates the printer DC only from validated or query-fallback DEVMODE data.
  [`windows.rs:200`](../../src-tauri/src/infrastructure/platform/printer_api/windows.rs#L200)

**Cross-platform geometry**

- Applies effective Landscape dimensions to macOS CUPS media generation.
  [`macos.rs:32`](../../src-tauri/src/infrastructure/platform/printer_api/macos.rs#L32)

- Applies the same orientation contract to Linux CUPS media generation.
  [`linux.rs:32`](../../src-tauri/src/infrastructure/platform/printer_api/linux.rs#L32)

**Regression coverage**

- Verifies propagation and independence between Landscape and PDF content rotation.
  [`bitmap_strategy.rs:204`](../../src-tauri/src/infrastructure/integrations/pdf_engine/bitmap_strategy.rs#L204)

- Verifies Windows orientation fields, fallback behavior, and private-data preservation.
  [`windows.rs:433`](../../src-tauri/src/infrastructure/platform/printer_api/windows.rs#L433)
