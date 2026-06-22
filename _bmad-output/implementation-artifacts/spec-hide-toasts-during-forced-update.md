---
title: 'Chỉ hiển thị modal khi bắt buộc cập nhật'
type: 'bugfix'
created: '2026-08-19'
status: 'done'
route: 'one-shot'
---

# Chỉ hiển thị modal khi bắt buộc cập nhật

## Intent

**Problem:** Toast lỗi về lần cập nhật chưa hoàn tất xuất hiện đồng thời với modal bắt buộc cập nhật, tạo hai thông báo chồng chéo cho cùng một trạng thái.

**Approach:** Không phát toast lỗi khi khôi phục trạng thái cập nhật, đồng thời tạm vô hiệu hóa và xóa toast trong lúc modal bắt buộc cập nhật đang hiển thị. Toast thành công được phát lại sau khi gate cập nhật đã đóng.

## Suggested Review Order

**Điều phối giao diện cập nhật**

- Gate cập nhật tắt toàn bộ toast và chỉ render modal bắt buộc.
  [`AppLayout.tsx:105`](../../src/components/AppLayout/AppLayout.tsx#L105)

- Nhánh khôi phục giữ modal nhưng không phát cảnh báo đỏ trùng lặp.
  [`AppLayout.tsx:43`](../../src/components/AppLayout/AppLayout.tsx#L43)

- Thông báo thành công chờ gate đóng rồi mới xuất hiện.
  [`AppLayout.tsx:53`](../../src/components/AppLayout/AppLayout.tsx#L53)

**Cơ chế chặn toast**

- Provider xóa hàng đợi và từ chối toast mới khi bị vô hiệu hóa.
  [`ToastProvider.tsx:14`](../../src/utils/toast/ToastProvider.tsx#L14)
