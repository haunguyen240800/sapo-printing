---
title: 'Chỉ sử dụng cổng local API 18901'
type: 'refactor'
created: '2026-08-24'
status: 'done'
route: 'one-shot'
---

# Chỉ sử dụng cổng local API 18901

## Intent

**Problem:** Local API vẫn thử các cổng `18902..=18910` khi `18901` bị chiếm, trong khi hợp đồng tích hợp yêu cầu một endpoint cố định.

**Approach:** Port binder chỉ bind `127.0.0.1:18901`; lỗi `AddrInUse` được trả thẳng và local API không khởi động trên cổng khác. Ví dụ client và tài liệu được đồng bộ để chỉ ping cổng `18901`.

## Suggested Review Order

**Fixed-port runtime**

- Binder công khai chỉ tạo listener tại endpoint cố định.
  [`port_binder.rs:7`](../../src-tauri/src/infrastructure/platform/port_binder.rs#L7)

- HTTP server không còn truyền hoặc nhận fallback range.
  [`server.rs:45`](../../src-tauri/src/interface/http_server/server.rs#L45)

- Test khóa contract chính xác `127.0.0.1:18901`.
  [`port_binder.rs:31`](../../src-tauri/src/infrastructure/platform/port_binder.rs#L31)

**Integration contract**

- Web client dùng một hằng cổng và một request discovery.
  [`webapp_integration.md:336`](../../docs/webapp_integration.md#L336)

- Tài liệu thiết kế mô tả fail-fast khi cổng bị chiếm.
  [`implementation_plan_sse.md:311`](../../docs/implementation_plan_sse.md#L311)
