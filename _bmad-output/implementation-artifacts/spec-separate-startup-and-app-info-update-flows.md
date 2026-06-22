---
title: 'Tách flow cập nhật startup và màn Thông tin'
type: 'bugfix'
created: '2026-08-19'
status: 'done'
route: 'one-shot'
---

# Tách flow cập nhật startup và màn Thông tin

## Intent

**Problem:** Kiểm tra cập nhật thủ công trong `AppInfoModal` broadcast kết quả tới `AppLayout`, khiến forced modal xuất hiện và làm flow cập nhật bên trong màn “Thông tin” mất tác dụng.

**Approach:** Chỉ kích hoạt forced update từ lần kiểm tra startup trực tiếp của layout hoặc pending marker sau restart. `checkForUpdates()` trở thành API thuần trả kết quả, nên `AppInfoModal` tự quản lý toàn bộ flow tải và cài đặt của nó. Pending reconciliation được xử lý trước startup check để tránh ghi đè trạng thái.

## Suggested Review Order

**Điều phối startup**

- Startup check được cache trong phiên và không nghe kết quả manual.
  [`AppLayout.tsx:16`](../../src/components/AppLayout/AppLayout.tsx#L16)

- Pending marker được đối chiếu trước khi áp dụng kết quả server.
  [`AppLayout.tsx:71`](../../src/components/AppLayout/AppLayout.tsx#L71)

**Ranh giới service**

- API kiểm tra chỉ trả kết quả, không broadcast sang flow khác.
  [`update-service.ts:23`](../../src/services/update-service.ts#L23)

- Pending marker chỉ bị xóa khi đúng target của lần cập nhật.
  [`update-service.ts:41`](../../src/services/update-service.ts#L41)
