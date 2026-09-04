# Investigation: Cấu hình Landscape nhưng bản in vẫn Portrait

## Hand-off Brief

1. **What happened.** `Landscape` được lưu đúng vào print job nhưng production bitmap path bỏ qua nó, backend contract không truyền orientation và Windows DEVMODE không được đặt Landscape.
2. **Where the case stands.** Root cause đã Confirmed với confidence High; hồ sơ Concluded, không còn nhánh source-code nào cần điều tra để triển khai bản sửa.
3. **What's needed next.** Dùng `bmad-quick-dev` để sửa đồng bộ bitmap geometry, graphics contract, Windows DEVMODE và tests.

## Case Info

| Field            | Value |
| ---------------- | ----- |
| Ticket           | N/A |
| Date opened      | 2026-09-04 |
| Status           | Concluded |
| System           | Windows printing backend, Tauri 2 / Rust |
| Evidence sources | Source code tại `printer_api` và `printer_command.rs` |

## Problem Statement

Người dùng báo: cấu hình "In chiều ngang" (`orientation = Landscape`) nhưng khi in ra vẫn theo chiều dọc.

## Evidence Inventory

| Source | Status | Notes |
| ------ | ------ | ----- |
| `src-tauri/src/infrastructure/platform/printer_api/windows.rs` | Available | Có xử lý DEVMODE orientation và fallback Portrait |
| `src-tauri/src/interface/tauri/commands/printer_command.rs` | Available | Có truyền/đọc trường `orientation` trong cấu hình UI |
| Source code caller/data flow | Available | Có `layout_engine.rs`, `printing.rs`, Windows DEVMODE và config adapters để truy vết |
| Tests liên quan orientation | Partial | Chỉ thấy assertion cho mặc định `Portrait`; chưa thấy test Landscape/DEVMODE |
| Version control | Partial | Repository chỉ có một commit `04de386`; không đủ lịch sử để xác định regression |
| Static/build artifacts | Available | Có `src-tauri/target`; chưa chạy kiểm tra vì Outcome 2 mới lập inventory |
| Runtime logs / printer model / driver | Missing | Chưa có dữ liệu thực địa để đối chiếu hành vi driver |
| Issue tracker / diagnostic archive | Missing | Người dùng không cung cấp ticket hoặc archive |

## Investigation Backlog

| # | Path to Explore | Priority | Status | Notes |
| - | --------------- | -------- | ------ | ----- |
| 1 | Truy vết orientation từ command/config sang print service | High | Done | Landscape được giữ đến job settings rồi bị bỏ qua trong production renderer/backend |
| 2 | Kiểm tra cách tạo và patch Windows DEVMODE | High | Done | Driver path giữ default; fallback ép Portrait; không có DMORIENT_LANDSCAPE |
| 3 | Kiểm tra test và lịch sử thay đổi liên quan | Medium | Done | Xác nhận coverage gap; dòng hiện hành bắt nguồn từ commit 04de386 |
| 4 | Phân biệt layout rotation với DEVMODE orientation | High | Done | Orientation điều khiển page geometry; rotate là phép xoay nội dung độc lập |

## Timeline of Events

| Time | Event | Source | Confidence |
| ---- | ----- | ------ | ---------- |
| 2026-09-04 | Báo lỗi Landscape in thành Portrait | User report | Deduced |
| 2026-09-04 | Static scan thấy fallback DEVMODE gán Portrait và code chủ ý giữ orientation từ driver | `src-tauri/src/infrastructure/platform/printer_api/windows.rs:55`, `:110-122`, `:147` | Confirmed |

## Confirmed Findings

### Finding 1: Windows DEVMODE không lấy orientation từ cấu hình tại điểm quét ban đầu

**Evidence:** `src-tauri/src/infrastructure/platform/printer_api/windows.rs:55-59`, `:110-122`, `:147`

**Detail:** Comment và phép gán cho thấy code giữ orientation từ driver; nhánh fallback gán rõ ràng Portrait.

## Deduced Conclusions

Chưa kết luận cho đến khi hoàn tất caller/data-flow trace.

## Hypothesized Paths

### Hypothesis 1: Giá trị Landscape không được áp dụng vào Windows DEVMODE

**Status:** Confirmed

**Theory:** Cấu hình UI có orientation nhưng print backend chỉ giữ orientation mặc định của driver hoặc fallback Portrait.

**Supporting indicators:** Static scan thấy trường orientation trong command, nhưng code DEVMODE được mô tả là giữ orientation của driver và fallback Portrait.

**Would confirm:** Caller chain không truyền orientation vào hàm tạo/patch DEVMODE, hoặc không gán `DMORIENT_LANDSCAPE` và flag `DM_ORIENTATION`.

**Would refute:** Có code được thực thi trước khi spool, gán đúng Landscape vào DEVMODE và driver chấp nhận giá trị đó.

**Resolution:** Caller-chain xác nhận `Landscape` được giữ nguyên đến `PrintJobSettings`, nhưng production `BitmapRenderStrategy` không đọc orientation; `GraphicsBackend::begin_document` không có tham số orientation; Windows backend không có `DMORIENT_LANDSCAPE` và chỉ gán Portrait trong fallback.

## Missing Evidence

| Gap | Impact | How to Obtain |
| --- | ------ | ------------- |
| Runtime DEVMODE ngay trước StartDoc/StartPage | Phân biệt lỗi mapping với driver bỏ qua | Thêm/log có mục tiêu hoặc test adapter |
| Model/driver máy in tái hiện lỗi | Kiểm tra hành vi đặc thù driver | Thu thập từ môi trường lỗi nếu static trace chưa đủ |

## Source Code Trace

| Element | Detail |
| ------- | ------ |
| Error origin | `src-tauri/src/infrastructure/integrations/pdf_engine/bitmap_strategy.rs:45-56` bỏ qua `settings.orientation`; `src-tauri/src/infrastructure/platform/printer_api/backend.rs:7-15` không có orientation trong contract; `src-tauri/src/infrastructure/platform/printer_api/windows.rs:54-122` không đặt Landscape |
| Trigger | Job có `settings.orientation = "Landscape"` đi qua production `BitmapRenderStrategy` |
| Condition | Driver trả default DEVMODE Portrait, hoặc `DocumentPropertiesW` thất bại và fallback ép Portrait |
| Related files | `bitmap_strategy.rs`, `backend.rs`, `windows.rs`, `macos.rs`, `linux.rs`, `app_state.rs`, `config_port.rs`, `layout_engine.rs`, `native_strategy.rs` |

## Conclusion

**Confidence:** High

Root cause đã được xác nhận. Giá trị `Landscape` đi đúng từ UI đến `PrintJobSettings`, nhưng production `BitmapRenderStrategy` không consume orientation và vẫn truyền kích thước portrait gốc. `GraphicsBackend::begin_document` không có orientation, nên Windows adapter chỉ giữ orientation mặc định của driver hoặc ép Portrait khi fallback. Vì vậy driver/HDC Portrait quyết định page geometry và bản in vẫn dọc.

Khả năng một driver có default Landscape vẫn còn Open ở cấp môi trường, nhưng chỉ có thể che triệu chứng trên thiết bị đó; nó không thay đổi kết luận rằng lựa chọn Landscape của ứng dụng hiện không điều khiển print backend.

## Recommended Next Steps

### Fix direction

1. Tạo representation orientation được validate (`Portrait`/`Landscape`) tại boundary phù hợp; không tiếp tục truyền chuỗi tự do xuống platform.
2. Tính effective paper dimensions theo orientation trong `BitmapRenderStrategy` để fallback và page geometry nhận width/height đúng.
3. Mở rộng `GraphicsBackend::begin_document` để truyền orientation; cập nhật Windows, macOS, Linux và test doubles.
4. Trong Windows `build_devmode`, thêm `DM_ORIENTATION` vào `dmFields` và gán `DMORIENT_LANDSCAPE`/`DMORIENT_PORTRAIT` ở cả driver-success lẫn fallback paths.
5. Giữ `orientation` độc lập với `rotate`; không tự biến Landscape thành phép quay nội dung 90°.
6. Thêm unit/integration tests cho mapping DEVMODE, effective dimensions, trait propagation và round-trip Landscape.

### Diagnostic

Sau khi sửa, log có mục tiêu ngay trước `CreateDCW`: requested orientation, `dmOrientation`, width/length và DC `HORZRES`/`VERTRES`. Không cần thêm logging lâu dài nếu tests và kiểm thử thiết bị đã chứng minh mapping.

## Reproduction Plan

1. Cấu hình khổ giấy không vuông và chọn Landscape.
2. In PDF có nội dung bất đối xứng, dễ nhận biết cạnh dài/ngắn.
3. Xác nhận fake backend nhận Landscape và effective width > height.
4. Trên Windows, xác nhận DEVMODE ngay trước `CreateDCW` có `DM_ORIENTATION` và `DMORIENT_LANDSCAPE`.
5. Xác nhận DC page geometry theo chiều ngang và bản in vật lý đúng Landscape.
6. Lặp lại với Portrait để chống regression; kiểm tra `rotate` 0/90 vẫn hoạt động độc lập.

## Side Findings

- Working tree có nhiều thay đổi không thuộc phạm vi điều tra; phải giữ nguyên và tránh ghi đè.

## Follow-up: 2026-09-04

### New Evidence

- Source scan xác nhận `src-tauri/src/domain/print_job/services/layout_engine.rs:9` có nhánh xử lý `LANDSCAPE`.
- Source scan xác nhận Windows backend tạo DEVMODE tại `src-tauri/src/infrastructure/platform/printer_api/windows.rs:54-157`.
- Test scan chỉ tìm thấy assertions cho mặc định Portrait tại `src-tauri/src/infrastructure/configs/app/app_print_config.rs:100,109`.
- Git history cho vùng code chỉ có commit `04de386` (`Initial commit`).
- Không có thư mục `logs` trong workspace và không có ticket/diagnostic archive được cung cấp.

### Additional Findings

Evidence perimeter đã đủ để truy vết tĩnh. Bằng chứng runtime và lịch sử regression đều thiếu; đây không ngăn cản việc xác định lỗi mapping nếu caller chain chứng minh Landscape không bao giờ được gán vào DEVMODE.

### Updated Hypotheses

Hypothesis 1 vẫn Open. Xuất hiện nhánh cần kiểm tra: hệ thống có thể chỉ hoán đổi kích thước render cho Landscape nhưng không đặt orientation của printer driver.

### Backlog Changes

Ưu tiên tiếp theo là đọc `layout_engine.rs`, `printing.rs`, `windows.rs`, config adapter và use case để lập caller chain chính xác.

### Updated Conclusion

**Confidence:** Low. Chưa đủ để kết luận root cause, nhưng code và coverage gap đều hỗ trợ tiếp tục trace tại boundary giữa layout engine và Windows DEVMODE.

## Follow-up: 2026-09-04 #2

### New Evidence

- UI tạo đúng `orientation = "Landscape"` và Tauri command lưu nguyên vẹn: `src/pages/printer/components/PrinterSettingsFormModal.tsx:174-191`, `src-tauri/src/interface/tauri/commands/printer_command.rs:56-72`.
- Config adapter, job snapshot, repository và queue đều giữ orientation: `src-tauri/src/infrastructure/configs/app/json_file_config_provider.rs:16-24`, `src-tauri/src/application/ports/config_port.rs:37-45`, `src-tauri/src/application/use_cases/create_print_job.rs:75-87`, `src-tauri/src/infrastructure/persistence/job_queue_broker.rs:101-129`.
- Production bootstrap dùng `BitmapRenderStrategy`: `src-tauri/src/bootstrap/app_state.rs:81-85`.
- `BitmapRenderStrategy` lấy kích thước portrait trực tiếp từ `PaperSize`, không đọc `settings.orientation`, và chỉ xoay theo `settings.rotate`: `src-tauri/src/infrastructure/integrations/pdf_engine/bitmap_strategy.rs:33-56,114-123`.
- `GraphicsBackend::begin_document` không mang orientation: `src-tauri/src/infrastructure/platform/printer_api/backend.rs:7-15`.
- Windows backend chỉ patch paper size, giữ orientation mặc định của driver; fallback đặt explicit Portrait: `src-tauri/src/infrastructure/platform/printer_api/windows.rs:54-97,110-122,135-157`.
- `LayoutEngine` có swap width/height cho Landscape nhưng chỉ được `NativePdfRenderStrategy` dùng; production không bootstrap strategy này: `src-tauri/src/domain/print_job/services/layout_engine.rs:6-10`, `src-tauri/src/infrastructure/integrations/pdf_engine/native_strategy.rs:37-49`, `src-tauri/src/bootstrap/app_state.rs:81-85`.

### Additional Findings

1. **Confirmed — config không làm mất Landscape.** Giá trị đi xuyên suốt frontend, Tauri, JSON, snapshot và durable queue.
2. **Confirmed — production renderer bỏ qua orientation.** Nó không swap paper dimensions theo Landscape và không suy ra rotation từ orientation.
3. **Confirmed — platform boundary làm mất khả năng điều khiển orientation.** Contract `begin_document` không truyền orientation, nên Windows backend không thể biết lựa chọn của người dùng.
4. **Confirmed — Windows DEVMODE không được đặt Landscape.** Không có `DMORIENT_LANDSCAPE`; driver-success path giữ default, fallback ép Portrait.

### Updated Hypotheses

- **Orientation bị mất khi lưu/load config — Refuted.** Trace từng phép gán không thấy Landscape bị đổi thành Portrait.
- **LayoutEngine xử lý Landscape trong production — Refuted.** Nhánh này thuộc native strategy chưa được bootstrap.
- **Landscape tự động rotate bitmap 90° — Refuted.** Rotation chỉ dùng `settings.rotate`, được conversion đặt `0.0`.
- **Windows nhận Landscape qua DEVMODE — Refuted.** Backend contract không mang orientation và repository không có `DMORIENT_LANDSCAPE`.
- **Driver default có thể là Landscape — Open.** Điều này có thể che lỗi trên một số máy nhưng không phản bác nguyên nhân: lựa chọn trong ứng dụng không điều khiển DEVMODE.

### Refutation Pass

Đã tìm cơ chế gián tiếp có thể phản bác root cause: frontend mapping, config round-trip, job serialization, queue restoration, LayoutEngine swap, bitmap rotation và driver DEVMODE. Chỉ config/job giữ đúng Landscape; không có cơ chế production nào chuyển nó thành page/DC orientation. Default DEVMODE Landscape của một driver chỉ làm lỗi không tái hiện trên máy đó, không biến lựa chọn ứng dụng thành input của backend.

### Backlog Changes

- Data-flow trace: Done.
- Windows DEVMODE trace: Done.
- Test/history inventory: Done; xác nhận coverage gap và thiếu lịch sử regression.
- Còn lại cho Outcome 4: chốt source trace và phân loại độ lớn của bản sửa.

### Updated Conclusion

**Confidence:** High. Root cause đã được xác nhận tĩnh: `Landscape` tồn tại trong job settings nhưng bị bỏ qua trong production bitmap path; backend contract không truyền orientation và Windows DEVMODE không bao giờ được đặt Landscape. Máy in dùng default Portrait vì thế tiếp tục tạo HDC/bản in Portrait.

## Follow-up: 2026-09-04 #3

### New Evidence

- `GraphicsBackend::begin_document` chỉ nhận printer, document, output path và kích thước giấy; không có orientation: `src-tauri/src/infrastructure/platform/printer_api/backend.rs:7-15`.
- Production renderer gọi contract bằng kích thước portrait gốc: `src-tauri/src/infrastructure/integrations/pdf_engine/bitmap_strategy.rs:45-56`.
- Printable area sau đó được lấy từ DC, nên HDC Portrait chi phối phép scale/blit: `src-tauri/src/infrastructure/integrations/pdf_engine/bitmap_strategy.rs:59-95,104-137`.
- Windows `build_devmode` chỉ patch paper fields ở driver-success path: `src-tauri/src/infrastructure/platform/printer_api/windows.rs:89-102`.
- Fallback explicit `DM_ORIENTATION | ...` nhưng gán `DMORIENT_PORTRAIT`: `src-tauri/src/infrastructure/platform/printer_api/windows.rs:110-125`.
- `CreateDCW` nhận chính DEVMODE này: `src-tauri/src/infrastructure/platform/printer_api/windows.rs:146-158`.
- `git -S`/blame cho thấy cấu trúc hiện tại có từ commit `04de386`; lịch sử trước đó không cho thấy implementation Landscape bị xóa trong các dòng hiện hành.

### Source Trace

1. UI/config/job boundary giữ đúng `Landscape`.
2. Tại `BitmapRenderStrategy`, orientation trở thành dữ liệu chết: code chỉ đọc `rotate` và `paper_size`.
3. Tại `GraphicsBackend`, signature không có cách biểu diễn orientation.
4. Tại Windows adapter, DEVMODE chỉ nhận kích thước giấy; orientation giữ default driver hoặc bị ép Portrait.
5. HDC báo printable area theo Portrait và bitmap được fit vào vùng đó, tạo đúng triệu chứng người dùng quan sát.

### Depth / Fix-size Assessment

Đây không phải lỗi một dòng. Sửa đúng cần thay đổi phối hợp ít nhất ba lớp và cập nhật mọi implementation của trait:

1. **Geometry:** tính effective paper dimensions theo orientation trong production bitmap strategy.
2. **Port contract:** truyền orientation bằng kiểu đã validate qua `GraphicsBackend::begin_document`.
3. **Windows adapter:** patch `DM_ORIENTATION` và map Portrait/Landscape cho cả driver-success và fallback paths.
4. **Cross-platform/tests:** cập nhật Linux/macOS implementations và fake backends/tests do thay đổi trait signature.

Không nên map `Landscape` thành `settings.rotate = 90`: orientation điều khiển page/DC geometry, còn rotate là phép xoay nội dung độc lập.

### Updated Conclusion

**Confidence:** High. Source trace đã hoàn tất tại nơi lỗi phát sinh và nơi biểu hiện. Không cần mở rộng sang printer discovery hoặc Tauri command để sửa root cause.
