# Installer packaging

Ứng dụng phát hành Windows bằng NSIS ở chế độ per-user (`currentUser` trong cấu hình Tauri). Installer chỉ ghi tài
nguyên trong phạm vi tài khoản hiện tại và không đăng ký service hay sửa system
trust store, vì vậy không cần quyền administrator.

Local API chạy plain HTTP và chỉ bind vào IPv4 loopback:

```text
http://127.0.0.1:18901/api/v1/ping
```

Nếu `18901` bận, app thử lần lượt `18902..=18910` và ghi cổng đã chọn vào
`agent.json`. Auto-update vẫn do Tauri updater thực hiện trong user session.
