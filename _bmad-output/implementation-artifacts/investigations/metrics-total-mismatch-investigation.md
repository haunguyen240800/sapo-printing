# Investigation: Chênh lệch tổng số job và số job kết thúc

## Hand-off Brief

1. **What happened.** Đã xác nhận UI hiển thị `total_jobs = 55`, `completed = 0`, `failed = 33`, trong khi backend định nghĩa tổng là mọi dòng của `print_jobs`.
2. **Where the case stands.** Có 22 job thuộc trạng thái khác `COMPLETED` và `FAILED`; phân bố trạng thái thực tế trong DB chưa được đọc.
3. **What's needed next.** Truy vấn DB development theo trạng thái và đối chiếu luồng recovery/worker để xác định vì sao 22 job còn tồn tại.

## Case Info

| Field | Value |
| --- | --- |
| Ticket | N/A |
| Date opened | 2026-09-04 |
| Status | Active |
| System | Windows; Tauri 2; SQLite development DB |
| Evidence sources | Ảnh người dùng, source frontend/backend, `config.db` development |

## Problem Statement

Người dùng quan sát màn hình hiển thị tổng 55, thành công 0, thất bại 33 và yêu cầu xác định nguyên nhân.

## Evidence Inventory

| Source | Status | Notes |
| --- | --- | --- |
| Ảnh màn hình | Available | Hiển thị 55 tổng, 0 thành công, 33 thất bại |
| Source metrics | Available | Truy vấn tổng và các trạng thái kết thúc |
| SQLite development DB | Available | `C:/Users/HauNV-PC/AppData/Roaming/sapo-printer-pro-max/config.db`; chưa truy vấn phân bố |
| Runtime logs | Partial | Vị trí biết được, chưa rà soát |

## Investigation Backlog

| # | Path to Explore | Priority | Status | Notes |
| - | --- | --- | --- | --- |
| 1 | Truy vấn `GROUP BY status` trong DB | High | Open | Xác định chính xác 22 job |
| 2 | Đối chiếu worker và startup recovery | High | Open | Giải thích nguyên nhân trạng thái tồn dư |
| 3 | Kiểm tra semantics mong muốn của nhãn `Tổng` | Medium | Open | Xác định đây là lỗi logic hay thiếu thông tin UI |

## Timeline of Events

| Time | Event | Source | Confidence |
| --- | --- | --- | --- |
| 2026-09-04 | UI hiển thị 55/0/33 | Ảnh người dùng | Confirmed |

## Confirmed Findings

### Finding 1: Tổng đếm toàn bộ job

**Evidence:** `src-tauri/src/infrastructure/telemetry/metrics.rs:78-87`

**Detail:** `completed` chỉ lấy `COMPLETED`, `failed` chỉ lấy `FAILED`, còn `total_jobs` dùng `COUNT(*)` không lọc trạng thái.

### Finding 2: Frontend không tự tính lại tổng

**Evidence:** `src/pages/printer/PrinterPage.tsx:126-130`

**Detail:** Frontend ánh xạ trực tiếp các trường metrics vào `stats`; `Overview` chỉ render các giá trị.

## Deduced Conclusions

### Deduction 1: Có 22 job ở trạng thái khác hai nhóm đang hiển thị

**Based on:** Finding 1 và ảnh màn hình.

**Reasoning:** `55 - 0 - 33 = 22`.

**Conclusion:** 22 job có thể đang chờ, đang xử lý hoặc đã hủy; cần DB để phân loại chính xác.

## Hypothesized Paths

### Hypothesis 1: 22 job là trạng thái không kết thúc hoặc CANCELLED

**Status:** Open

**Theory:** Tổng bao gồm mọi trạng thái nhưng UI chỉ hiển thị hai loại `COMPLETED` và `FAILED`.

**Supporting indicators:** SQL metrics đã xác nhận phạm vi đếm không đồng nhất.

**Would confirm:** Tổng count của các trạng thái ngoài `COMPLETED`/`FAILED` bằng 22.

**Would refute:** DB không có 22 dòng như vậy hoặc UI đang đọc DB khác.

**Resolution:** Chưa có.

## Missing Evidence

| Gap | Impact | How to Obtain |
| --- | --- | --- |
| Phân bố trạng thái DB | Không biết 22 job cụ thể là gì | Truy vấn read-only `GROUP BY status` |
| Log của các job tồn dư | Chưa biết cơ chế tạo trạng thái | Đối chiếu timestamps và runtime logs |

## Source Code Trace

| Element | Detail |
| --- | --- |
| Error origin | `src-tauri/src/infrastructure/telemetry/metrics.rs:78-87`, semantics metrics |
| Trigger | UI gọi Tauri command `get_metrics` |
| Condition | DB chứa job ngoài `COMPLETED` và `FAILED` |
| Related files | `src/pages/printer/PrinterPage.tsx`, `src/pages/printer/components/Overview.tsx` |

## Conclusion

**Confidence:** Medium

Đã xác nhận nguyên nhân trực tiếp của phép tính lệch là phạm vi trạng thái khác nhau. Chưa xác nhận phân bố và nguồn gốc của 22 job còn lại.

## Recommended Next Steps

### Fix direction

Chưa đề xuất thay đổi cho đến khi xác định semantics sản phẩm của nhãn `Tổng`.

### Diagnostic

Truy vấn phân bố trạng thái và timestamps trong DB development, sau đó đối chiếu recovery và queue worker.

## Reproduction Plan

Tạo các job ở trạng thái kết thúc và không kết thúc; gọi `get_metrics`; xác nhận `total_jobs` lớn hơn `completed + failed`.

## Side Findings

- Worktree hiện có nhiều thay đổi chưa commit; điều tra chỉ đọc và không chạm vào các file source đó.

## Follow-up: 2026-09-04

### New Evidence

- Truy vấn read-only DB development hiện tại trả về `CANCELLED = 10`, `COMPLETED = 105`, tổng 115; không có `FAILED` hay trạng thái đang xử lý.
- 10 job `CANCELLED` được tạo trong hai cụm thời gian và đều có `retry_count = 0`, `error_message = NULL`.
- Runtime logs có đủ bốn mức `debug/info/warn/error`, cập nhật gần nhất ngày 2026-09-03; chưa đọc nội dung vì bước này chỉ lập evidence perimeter.
- Lịch sử Git của vùng liên quan chỉ có commit nền `04de386`; file recovery hiện là thay đổi chưa commit.
- Source xác nhận startup recovery chủ động đổi `PENDING/QUEUED` cũ thành `CANCELLED` tại `src-tauri/src/application/use_cases/recover_interrupted_jobs.rs:18-19`.

### Additional Findings

1. **Confirmed:** DB hiện tại không còn khớp ảnh 55/0/33. Ảnh phản ánh snapshot cũ, một DB/runtime khác, hoặc metrics tại thời điểm khác.
2. **Confirmed:** Trạng thái `CANCELLED` vẫn được tính vào `total_jobs` nhưng không được tính vào `completed` hay `failed`.
3. **Partial:** Có cơ chế recovery đủ khả năng tạo job `CANCELLED`, nhưng chưa đối chiếu event/log để chứng minh chính cơ chế này tạo ra 10 job hiện tại.

### Updated Hypotheses

#### Hypothesis 1: Các job chênh lệch là trạng thái ngoài COMPLETED/FAILED

**Status:** Confirmed về cơ chế, chưa thể gắn trực tiếp với snapshot 55/0/33.

**Resolution:** DB hiện tại chứng minh `CANCELLED` làm `total_jobs` lớn hơn `completed + failed`; tuy nhiên dữ liệu đã thay đổi từ khi chụp ảnh nên không thể xác nhận 22 job cũ là trạng thái gì nếu không có snapshot/log tương ứng.

#### Hypothesis 2: Startup recovery tạo các job CANCELLED

**Status:** Open.

**Would confirm:** Event hoặc log tại timestamps của 10 job cho thấy `RecoverInterruptedJobsUseCase` chuyển chúng từ `PENDING/QUEUED` sang `CANCELLED`.

**Would refute:** Event audit cho thấy API cancel hoặc luồng khác thực hiện transition.

### Backlog Changes

| # | Path to Explore | Priority | Status | Notes |
| - | --- | --- | --- | --- |
| 1 | Truy vấn phân bố trạng thái DB | High | Done | DB hiện tại: 105 COMPLETED, 10 CANCELLED |
| 2 | Đối chiếu events/log cho 10 job CANCELLED | High | Open | Phân biệt startup recovery và cancel chủ động |
| 3 | Đối chiếu worker/recovery source | High | In Progress | Đã thấy recovery rule, chưa lần caller/timeline |
| 4 | Xác định snapshot 55/0/33 | Medium | Blocked | DB hiện tại đã thay đổi; cần DB backup hoặc log cùng thời điểm |
| 5 | Chốt semantics UI của `Tổng` | Medium | Open | Tổng mọi job hay chỉ terminal job |

### Evidence Perimeter

| Category | Status | Detail |
| --- | --- | --- |
| Diagnostic database | Available | Đã truy vấn read-only; hiện có 115 job |
| Runtime logs | Available | 4 file log, khoảng 1.8 MB; chưa đọc nội dung |
| Issue tracker | Missing | Không có ticket tham chiếu |
| Version control | Partial | Chỉ có initial commit; recovery là thay đổi chưa commit |
| Test results | Missing | Chưa chạy test trong pha lập bản đồ bằng chứng |
| Static analysis | Partial | Có test metrics/recovery trong source, chưa chạy |
| Source code | Available | Metrics, frontend mapping, recovery và persistence đã định vị |

### Updated Conclusion

**Confidence:** High cho nguyên nhân hiển thị; Low cho thành phần chính xác của 22 job tại thời điểm chụp ảnh.

Chênh lệch là hành vi tất định: `Tổng` bao gồm cả `CANCELLED` và trạng thái chưa kết thúc, trong khi hai ô còn lại chỉ hiển thị `COMPLETED`/`FAILED`. DB hiện tại đã thay đổi thành 115 job nên không còn bằng chứng trực tiếp để phân loại 22 job trong ảnh.
