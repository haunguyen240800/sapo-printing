# Investigation: Trạng thái runtime của cấu hình image và buffer

## Hand-off Brief

1. **Điều cần xác minh.** Bốn trường `print_as_image`, `color_mode`, `enable_buffer`, `buffer_size_kb` có được tiêu thụ trong pipeline in hay chỉ được validate/lưu cấu hình.
2. **Tình trạng hiện tại.** Active; đã xác nhận Tauri command nhận và ghi cả bốn trường, nhưng chưa truy vết đầy đủ tới renderer/spooler.
3. **Bước tiếp theo.** Theo caller/data flow từ JSON config qua snapshot `PrintJobSettings` tới từng print strategy và graphics backend.

## Case Info

| Field | Value |
| --- | --- |
| Ticket | N/A |
| Date opened | 2026-09-04 |
| Status | Active |
| System | Sapo Printer Pro Max; Rust/Tauri; production bitmap printing |
| Evidence sources | Source code, tests, Git history |

## Problem Statement

Người dùng yêu cầu debug xem các trường `PrinterConfigRequest.print_as_image`, `color_mode`, `enable_buffer`, `buffer_size_kb` hiện đã được xử lý khi in hay chưa.

## Evidence Inventory

| Source | Status | Notes |
| --- | --- | --- |
| `printer_command.rs` | Available | Stronghold: command validate/map cả bốn trường tại dòng 48-69. |
| Config adapters/domain settings | Partial | Grep cho thấy snapshot domain chỉ khai báo image/color, cần đọc caller chain. |
| Renderer/spooler/backend | Partial | Chưa xác nhận consumer runtime. |
| Runtime logs/hardware observation | Missing | Không cần để chứng minh static data flow; cần nếu hành vi phụ thuộc driver. |

## Investigation Backlog

| # | Path to Explore | Priority | Status | Notes |
| - | --- | --- | --- | --- |
| 1 | Request → JSON config → ConfigPort snapshot | High | Open | Xác định trường bị loại khỏi snapshot. |
| 2 | Snapshot → renderer/print service/backend | High | Open | Tìm consumer thực tế của từng trường. |
| 3 | Tests và Git history | Medium | Open | Phân biệt intentional/dead configuration. |

## Timeline of Events

| Time | Event | Source | Confidence |
| --- | --- | --- | --- |
| 2026-09-04 | Yêu cầu kiểm tra bốn trường cấu hình | User report | Confirmed |

## Confirmed Findings

### Finding 1: Tauri command nhận và map cả bốn trường

**Evidence:** `src-tauri/src/interface/tauri/commands/printer_command.rs:48`, `src-tauri/src/interface/tauri/commands/printer_command.rs:66`

**Detail:** Command validate `color_mode` và xây `AppPrintConfig` chứa `color_mode`, `print_as_image`, `enable_buffer`, `buffer_size_kb`.

## Deduced Conclusions

Chưa kết luận trước khi hoàn tất caller/data-flow trace.

## Hypothesized Paths

### Hypothesis 1: Một hoặc nhiều trường chỉ được lưu nhưng chưa tác động lệnh in

**Status:** Open

**Theory:** Các trường tồn tại ở request/config response nhưng không có consumer trong renderer hoặc graphics backend.

**Supporting indicators:** Exact-symbol scan chỉ thấy `enable_buffer`/`buffer_size_kb` ở UI, command và cấu hình JSON; chưa thấy trong `PrintJobSettings` hay print backend.

**Would confirm:** Caller chain cho thấy trường bị loại khỏi snapshot hoặc không được đọc sau khi snapshot.

**Would refute:** Có consumer runtime sử dụng trường để chọn strategy, pixel format hoặc buffering.

**Resolution:** Pending.

## Missing Evidence

| Gap | Impact | How to Obtain |
| --- | --- | --- |
| Full caller/data-flow trace | Chưa thể kết luận từng trường | Đọc các file config, use case, renderer và backend liên quan. |

## Source Code Trace

| Element | Detail |
| --- | --- |
| Error origin | Chưa xác định; đây là kiểm tra area/runtime behavior |
| Trigger | Lưu `PrinterConfigRequest`, sau đó tạo và xử lý print job |
| Condition | Config field được lưu nhưng có thể không được snapshot/consume |
| Related files | `printer_command.rs`, `config_port.rs`, `settings.rs`, bitmap/native strategies |

## Conclusion

**Confidence:** Low

Đã xác nhận đầu vào command, chưa đủ bằng chứng để kết luận bốn trường có hiệu lực khi in.

## Recommended Next Steps

### Fix direction

Chỉ xác định sau khi hoàn tất điều tra; chưa sửa code trong workflow này.

### Diagnostic

Truy vết static data flow và test hiện có cho từng trường.

## Reproduction Plan

So sánh snapshot/job và lời gọi renderer/backend khi lần lượt bật từng cấu hình.

## Side Findings

- Chưa có.
