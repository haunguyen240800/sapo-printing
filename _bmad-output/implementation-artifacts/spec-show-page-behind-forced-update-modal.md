---
title: 'Hiển thị trang phía sau modal cập nhật bắt buộc'
type: 'bugfix'
created: '2026-08-19'
status: 'done'
route: 'one-shot'
---

# Hiển thị trang phía sau modal cập nhật bắt buộc

## Intent

**Problem:** Khi có bản cập nhật bắt buộc, layout bỏ render trang hiện tại nên phía sau modal chỉ còn một nền xám trống.

**Approach:** Giữ route hiện tại được render phía sau overlay, nhưng đặt phần nền ở trạng thái `inert` và ẩn khỏi accessibility tree để modal cập nhật vẫn chặn thao tác. Dialog ghép nối, toast và phím Escape nền tiếp tục bị vô hiệu hóa.

## Suggested Review Order

**Hiển thị nền có kiểm soát**

- Route luôn được render để nội dung hiện phía sau overlay.
  [`AppLayout.tsx:125`](../../src/components/AppLayout/AppLayout.tsx#L125)

- Modal cập nhật và dialog ghép nối loại trừ lẫn nhau.
  [`AppLayout.tsx:128`](../../src/components/AppLayout/AppLayout.tsx#L128)

**Khóa tương tác nền**

- `inert` và `aria-hidden` giữ nền chỉ mang tính trực quan.
  [`AppLayout.tsx:26`](../../src/components/AppLayout/AppLayout.tsx#L26)

- Escape bị chặn trước listener của các overlay nền.
  [`AppLayout.tsx:35`](../../src/components/AppLayout/AppLayout.tsx#L35)
