---
title: 'Cải thiện hướng dẫn cài đặt bản cập nhật bắt buộc'
type: 'chore'
created: '2026-08-19'
status: 'done'
route: 'one-shot'
---

# Cải thiện hướng dẫn cài đặt bản cập nhật bắt buộc

## Intent

**Problem:** Nội dung ở trạng thái đã tải xong chưa diễn đạt gọn và rõ các bước người dùng cần thực hiện để cài đặt bản cập nhật.

**Approach:** Hướng dẫn người dùng mở trình cài đặt, xác nhận UAC, chờ hoàn tất và cho biết ứng dụng sẽ tự động mở lại.

## Suggested Review Order

- Microcopy mô tả đúng chuỗi thao tác cài đặt và kết quả mong đợi.
  [`ForcedUpdateModal.tsx:166`](../../src/components/ForcedUpdateModal.tsx#L166)
