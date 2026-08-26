# Plan: Chuyển metrics từ polling sang event-driven thuần

## Bối cảnh

Hiện `PrinterPage` poll Tauri command `get_metrics` mỗi 5s (`setInterval`), gây log spam
(`GetMetricsUseCase: starting/completed` liên tục) và query DB không cần thiết. Metrics
(`total_jobs`, `completed`, `failed`, `last_print_time_secs`) chỉ đổi khi một job đạt trạng
thái terminal — mà UI **đã** nhận đúng các thời điểm đó qua Tauri event `onJobStatusChanged`.

## Mục tiêu

Bỏ hẳn interval polling. Chỉ load metrics:
1. Một lần lúc mount (initial load) — giữ nguyên.
2. Mỗi khi nhận job status event ở trạng thái terminal (`COMPLETED` / `FAILED` / `CANCELLED`).

Không giữ fallback polling (theo yêu cầu: event-driven thuần).

## Phạm vi thay đổi

Chỉ 1 file: `src/pages/printer/PrinterPage.tsx`.

Backend không đổi: emitter đã emit `PrintJobCompleted` / `PrintJobFailed`
(`src-tauri/src/interface/tauri/print_job_event_emitter.rs:14-15`), map sang status
`COMPLETED` / `FAILED` cho UI. Không cần thêm event mới.

## Chi tiết chỉnh sửa (PrinterPage.tsx)

1. **Xóa interval** (dòng ~60-63):
   ```ts
   metricsIntervalRef.current = window.setInterval(() => {
     loadMetrics();
   }, 5000);
   ```
   Giữ lại lời gọi `loadMetrics()` initial ở dòng 58.

2. **Xóa ref không còn dùng**: `metricsIntervalRef` (khai báo dòng 27) và block cleanup
   dòng 104-106 (`if (metricsIntervalRef.current) clearInterval(...)`).

3. **Gọi `loadMetrics()` khi job vào trạng thái terminal**: trong callback
   `onJobStatusChanged`, tại nhánh terminal hiện có (dòng 79):
   ```ts
   if (payload.status === "COMPLETED" || payload.status === "FAILED" || payload.status === "CANCELLED") {
     loadMetrics(); // <-- thêm: refresh số liệu ngay khi job kết thúc
     // ... phần schedule removal giữ nguyên
   }
   ```
   Lưu ý: `loadMetrics` đã nằm trong dependency array của `useEffect` (dòng 112) nên không
   phát sinh cảnh báo lint mới.

## Ảnh hưởng / rủi ro

- **Ưu**: không còn call `get_metrics` mỗi 5s → hết log spam + giảm tải DB; UI vẫn cập nhật
  tức thì vì event tới ngay khi job xong.
- **Rủi ro đã chấp nhận (không có fallback)**: nếu một job kết thúc mà UI **không** nhận
  được Tauri event (ví dụ emit thất bại, hoặc job chạy khi app đóng), metrics sẽ chỉ đồng
  bộ lại ở lần mở/mount trang kế tiếp. Đây là đánh đổi có chủ đích theo yêu cầu event-driven
  thuần.

## Kiểm thử

- `pnpm run build:web` (TypeScript check + build).
- `pnpm lint`.
- Thủ công: tạo print job, xác nhận stats (total/success/failed/printTime) cập nhật khi job
  hoàn tất mà không còn thấy log `GetMetricsUseCase` chạy mỗi 5s.

## Câu hỏi mở

- Có muốn thêm reload metrics khi window focus (rẻ, che được case lệch số liệu) không? Theo
  yêu cầu hiện tại là **không**, nhưng ghi ra đây để cân nhắc.
