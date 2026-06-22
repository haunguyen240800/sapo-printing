---
title: 'Ngăn modal con chồng lên luồng cập nhật bắt buộc'
type: 'bugfix'
created: '2026-08-19'
status: 'done'
route: 'one-shot'
---

# Ngăn modal con chồng lên luồng cập nhật bắt buộc

## Intent

**Problem:** Khi màn “Thông tin” đang mở và phát hiện phiên bản mới, `ForcedUpdateModal` xuất hiện nhưng modal cũ vẫn tồn tại phía dưới, tạo giao diện hai modal chồng nhau.

**Approach:** Truyền trạng thái forced update qua Outlet context; trang máy in đóng state modal hiện tại, không render hoặc mở modal con nào trong thời gian gate cập nhật hoạt động. Trang chính vẫn hiển thị phía sau overlay.

## Suggested Review Order

**Nguồn trạng thái gate**

- Layout truyền forced-update state đồng bộ cho route đang hiển thị.
  [`AppLayout.tsx:126`](../../src/components/AppLayout/AppLayout.tsx#L126)

**Loại trừ modal con**

- Trang xóa state modal và chặn mọi đường mở mới khi gate bật.
  [`PrinterPage.tsx:21`](../../src/pages/printer/PrinterPage.tsx#L21)

- Tất cả modal con chỉ được render ngoài trạng thái forced update.
  [`PrinterPage.tsx:158`](../../src/pages/printer/PrinterPage.tsx#L158)
