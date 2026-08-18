# HTTPS Local Server với Self-signed Certificate

## Mô tả

Thêm HTTPS local server vào `sapo-printing` desktop app để webapp (HTTPS) giao tiếp trực tiếp với desktop app, không bị chặn bởi Mixed Content policy. Server expose REST API nhận print job và SSE endpoint push trạng thái in ngược về webapp — hoạt động trên tất cả browser kể cả Safari.

---

## Quyết định kiến trúc (đã cập nhật)

> [!IMPORTANT]
> **Hostname**: Dùng `local.mysapo.net` (public DNS A record → `127.0.0.1`) thay vì `localhost`.
> **Lý do**: Safari + một số version Chrome có edge case với mixed-content khi target là `localhost` từ HTTPS parent origin. Public DNS trỏ loopback là pattern chuẩn (Plex, Discord, Zoom dùng). Server cert SAN: `local.mysapo.net`, `localhost`, `127.0.0.1` để fallback.

> [!IMPORTANT]
> **Cert lifetime**:
> - CA: 10 năm (root, tự sinh, install 1 lần).
> - Server (leaf): **397 ngày** (giới hạn browser hiện đại — Chrome/Safari reject leaf > 398 ngày kể cả private CA). Auto-renew khi còn < 30 ngày.

> [!IMPORTANT]
> **Cert store scope — system-wide (all users)**:
> - Windows: `LocalMachine\Root` — cần UAC elevation **1 lần lúc installer chạy**.
> - macOS: `/Library/Keychains/System.keychain` — pkg installer request admin password 1 lần.
> - Linux: `/usr/local/share/ca-certificates/` + `update-ca-certificates` — deb/rpm postinst chạy root.
> - App runtime KHÔNG cần admin. Chỉ installer + helper service cần elevated.

> [!IMPORTANT]
> **CA private key security**:
> - Sinh CA lúc **first-run trên máy customer** (KHÔNG ship kèm installer — tránh single-point-of-failure toàn hệ thống nếu key rò).
> - Lưu `ca.key` machine-wide: `C:\ProgramData\SapoPrinter\tls\ca.key` / `/Library/Application Support/SapoPrinter/tls/ca.key` / `/var/lib/sapo-printer/tls/ca.key`.
> - Mode `0600`, owner SYSTEM/root. Chỉ helper service đọc được.

> [!IMPORTANT]
> **Cert renewal — Elevated helper service**:
> - Service background (Windows Service / launchd daemon / systemd service) chạy SYSTEM/root.
> - Loop 24h check server cert expiry. Renew khi < 30 ngày.
> - App runtime (user-level) đọc `server.pem` + `server.key` (mode `0640`, group readable).
> - File watch (notify crate) → detect cert mới → hot reload rustls acceptor không restart app.
> - IPC socket (named pipe / unix socket) cho app trigger `renew_now` on-demand.

> [!IMPORTANT]
> **Port**: Cố định `18901`. Nếu bị chiếm → fallback range `18901–18910`. Port đã chọn ghi vào `~/.sapo-printer/agent.json`. Webapp discover qua `/api/v1/ping` với retry range.

> [!IMPORTANT]
> **Token security**: Token chỉ trả plaintext 1 lần lúc pair. DB lưu SHA-256 hash + salt. Token có `expires_at` (default 90 ngày, sliding window: refresh khi `last_used_at` update).

---

## Open Questions

- [ ] Xác nhận `local.mysapo.net` DNS record (A → 127.0.0.1) đã publish ổn định.
- [ ] Đường link tải desktop app cho `AgentSetupGuide` (Windows/macOS/Linux installers).

---

## Proposed Changes

---

### Component 1: Rust Dependencies

#### [MODIFY] `src-tauri/Cargo.toml`

```toml
# TLS & HTTPS Server
rcgen = "0.13"
axum = { version = "0.7", features = ["macros"] }
axum-server = { version = "0.7", features = ["tls-rustls"] }
rustls = "0.23"
tokio-rustls = "0.26"
tokio-stream = { version = "0.1", features = ["sync"] }
tower-http = { version = "0.5", features = ["cors", "trace"] }
tower_governor = "0.4"          # Rate limit /pair
sha2 = "0.10"                   # Token hashing
subtle = "2.5"                  # Constant-time compare
url = "2.5"                     # CORS Origin parsing
```

---

### Component 2: Certificate Management

#### [NEW] `src/infrastructure/tls/cert_generator.rs`

```rust
pub struct CertBundle {
    pub ca_cert_pem: String,
    pub server_cert_pem: String,
    pub server_key_pem: String,
    pub is_newly_generated: bool,
    pub ca_expires_at: SystemTime,
    pub server_expires_at: SystemTime,
}

impl CertGenerator {
    pub fn load_or_generate(data_dir: &Path) -> Result<CertBundle, TlsError>;
    pub fn renew_server_cert(data_dir: &Path, ca: &CaCert) -> Result<CertBundle, TlsError>;
}
```

- **CA cert**: 10 năm, `CN=Sapo Printer Local CA`, `basicConstraints=CA:TRUE`, `keyUsage=keyCertSign,cRLSign`.
- **Server cert**: **397 ngày**, `CN=local.mysapo.net`, SAN: `local.mysapo.net`, `localhost`, `127.0.0.1`, `::1`. `extKeyUsage=serverAuth`.
- Lưu: `~/.sapo-printer/tls/ca.pem`, `ca.key`, `server.pem`, `server.key`. File mode `0600`.
- CA key mã hóa AES-256 với key từ OS secret store (WindowsCredentialManager / Keychain / SecretService). Nếu store fail → cảnh báo user, key plaintext (fallback dev only).

#### [NEW] `src/infrastructure/tls/cert_installer.rs` — chạy TRONG helper service (elevated)

**Windows** — LocalMachine scope, needs SYSTEM/admin:
```rust
use windows::Win32::Security::Cryptography::*;
let store = CertOpenStore(
    CERT_STORE_PROV_SYSTEM_W,
    0,
    HCRYPTPROV_LEGACY::default(),
    CERT_SYSTEM_STORE_LOCAL_MACHINE_ID,
    Some("Root".to_wide().as_ptr() as _),
)?;
CertAddCertificateContextToStore(store, cert_ctx, CERT_STORE_ADD_REPLACE_EXISTING, None)?;
```
Hoặc gọi `certutil -addstore -f "Root" ca.pem` (nếu Wix custom action).

**macOS** — System.keychain:
```rust
Command::new("security")
    .args([
        "add-trusted-cert", "-d", "-r", "trustRoot",
        "-k", "/Library/Keychains/System.keychain",
        ca_cert_path,
    ])
    .status()
```
Chạy trong pkg `postinstall` (đã root).

**Linux** — system trust:
```bash
cp ca.pem /usr/local/share/ca-certificates/sapo-printer-ca.crt
update-ca-certificates
# NSS shared (Chrome/Firefox modern distros):
trust anchor /var/lib/sapo-printer/tls/ca.pem
```
Chạy trong deb/rpm postinst.

**Uninstall path**:
```rust
pub fn uninstall_ca() -> Result<(), TlsError>;
```
Gọi trong installer uninstall hook (elevated). Windows: `CertDeleteCertificateFromStore` LocalMachine\Root. macOS: `security delete-certificate -c "Sapo Printer Local CA" /Library/Keychains/System.keychain`. Linux: xóa `/usr/local/share/ca-certificates/sapo-printer-ca.crt` + `update-ca-certificates --fresh`.

#### [NEW] `src/infrastructure/tls/cert_checker.rs`

```rust
pub fn is_ca_trusted() -> bool;                              // Verify chain to system trust
pub fn needs_renewal(bundle: &CertBundle) -> RenewalStatus;
pub enum RenewalStatus { Ok, RenewSoon, Expired, CaRotationNeeded }
// RenewSoon: server < 30 ngày. CaRotationNeeded: CA < 60 ngày.
```

Renewal chạy trong **helper service** (không phải app). App chỉ file-watch `server.pem` và hot-reload rustls acceptor (`RustlsConfig::reload_from_pem_file`).

---

### Component 3: Authentication — Token-based + Auto-pairing

#### Flow

```
Lần đầu:
  1. Webapp POST /api/v1/pair { origin }  (không auth, rate limit 3 req/phút/origin)
  2. Server validate Origin header khớp body.origin, phải là https://*.mysapo.net
  3. Desktop hiện toast: "admin.mysapo.net muốn kết nối in ấn. [Từ chối] [Cho phép]"
  4. Timeout 60s. User Allow → server tạo token (32 bytes random), hash SHA-256+salt, lưu DB
  5. Trả plaintext token 1 lần: { api_token, expires_at }
  6. Webapp lưu localStorage. Các lần sau: Authorization: Bearer <token>
```

#### [NEW] `src/infrastructure/tls/api_token.rs`

```rust
pub struct ApiTokenManager {
    db_pool: DbPool,
    pending_pair_tx: tokio::sync::watch::Sender<Option<PairRequest>>,
    rate_limiter: DashMap<String, RateWindow>, // per-origin
}

impl ApiTokenManager {
    pub async fn request_pair(&self, origin: &str) -> Result<TokenResponse, PairError>;
    pub fn verify_token(&self, presented: &str) -> Option<TokenRecord>;  // Constant-time compare
    pub fn touch_last_used(&self, token_hash: &str);                     // Sliding expiry
    pub fn revoke_token(&self, token_hash: &str) -> Result<(), String>;
    pub fn list_paired_origins(&self) -> Vec<PairedOrigin>;
}

pub struct TokenResponse {
    pub api_token: String,        // Plaintext, trả 1 lần
    pub expires_at: i64,
}
```

**Verify** dùng `subtle::ConstantTimeEq` so hash — tránh timing attack.

#### SQLite migration mới

```sql
CREATE TABLE api_tokens (
    token_hash TEXT PRIMARY KEY,   -- SHA-256(token || salt), hex
    token_salt TEXT NOT NULL,
    origin TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    last_used_at INTEGER,
    expires_at INTEGER NOT NULL,   -- Sliding: touch update +90 ngày
    is_active INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_tokens_origin ON api_tokens(origin);
```

#### Tauri commands

- `get_paired_origins` — Desktop UI hiển thị + Revoke button.
- `revoke_token(token_hash)` — Xóa row.
- `get_agent_port` — Trả port đang bind (webapp fallback discovery).

---

### Component 4: HTTPS Local Server

#### CORS — parse Origin đúng cách

```rust
use url::Url;

let cors = CorsLayer::new()
    .allow_origin(AllowOrigin::predicate(|origin_hdr, _req| {
        let Ok(s) = origin_hdr.to_str() else { return false; };
        let Ok(url) = Url::parse(s) else { return false; };
        if url.scheme() != "https" { return false; }
        let Some(host) = url.host_str() else { return false; };
        host == "mysapo.net" || host.ends_with(".mysapo.net")
    }))
    .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
    .allow_headers([CONTENT_TYPE, AUTHORIZATION])
    .allow_credentials(false)
    .max_age(Duration::from_secs(3600));
```

Không so byte thô — parse URL, check scheme + host chính xác.

#### Rate limit `/api/v1/pair`

```rust
// tower_governor: 3 req / phút / IP
let pair_governor = GovernorLayer {
    config: Arc::new(
        GovernorConfigBuilder::default()
            .per_second(20)
            .burst_size(3)
            .finish()
            .unwrap(),
    ),
};
```

Cộng thêm per-origin limit trong `ApiTokenManager` (3 pending pair/origin/hour).

#### [NEW] `src/interface/http_server/router.rs`

| Method | Endpoint | Auth | Mô tả |
|--------|----------|------|-------|
| GET | `/api/v1/ping` | Không | Health check + version + supported_features |
| POST | `/api/v1/pair` | Origin check + rate limit | Pairing — chờ user confirm |
| POST | `/api/v1/jobs` | Bearer token | Tạo print job(s) |
| GET | `/api/v1/jobs/{id}` | Bearer token | Trạng thái job |
| GET | `/api/v1/printers` | Bearer token | Danh sách máy in |
| GET | `/api/v1/events` | `?token=xxx` | SSE stream job status |

**`/api/v1/ping` response** (version-aware):
```json
{ "status": "ok", "version": "1.2.3", "min_webapp_version": "2.0.0", "features": ["sse", "pair"] }
```
Webapp check `min_webapp_version` để hiện update notice.

**SSE query token — logging policy**:
- Global `tower_http::trace` layer scrub `token` query param trước khi log.
- Custom `MakeSpan` mask: `?token=***`.

#### [NEW] `src/interface/http_server/handlers/sse.rs`

```rust
pub struct SseBroadcaster {
    tx: broadcast::Sender<SseJobEvent>,
}

impl EventHandler for SseBroadcaster {
    fn handle(&self, event_type: &str, payload: &str) {
        let event = SseJobEvent::from_domain(event_type, payload);
        let _ = self.tx.send(event);
    }
}

// SSE frames:
// event: job_status
// data: {"job_id":"uuid","status":"COMPLETED","progress":100}
//
// event: ping        (mỗi 15s để phát hiện dead connection)
// data: {}
```

Reconnect: server hỗ trợ `Last-Event-ID` — replay events từ ring buffer (giữ 100 event gần nhất).

---

### Component 5: Port Discovery & Fallback

#### [NEW] `src/infrastructure/tls/port_binder.rs`

```rust
pub fn bind_with_fallback(preferred: u16, range: RangeInclusive<u16>)
    -> Result<(TcpListener, u16), std::io::Error>;
```

- Thử `preferred=18901` trước.
- Fail (EADDRINUSE) → thử `18902..=18910`.
- Ghi port đã chọn vào `~/.sapo-printer/agent.json`:
  ```json
  { "port": 18901, "version": "1.2.3", "started_at": 1710000000 }
  ```
- Tauri command `get_agent_port` trả port hiện tại (cho UI).
- Webapp discovery: loop ping `18901..=18910` với timeout ngắn (200ms mỗi port), cache port thành công vào `localStorage`.

---

### Component 5b: Elevated Helper Service — Cert Lifecycle Daemon

**Binary riêng**: `sapo-printer-cert-manager` (không chạy chung process với app).

**Đăng ký service**:
- Windows: Windows Service (`sc create SapoPrinterAgent binPath=... start=auto`), chạy `LocalSystem`.
- macOS: launchd daemon (`/Library/LaunchDaemons/com.sapo.printer.agent.plist`), chạy `root`.
- Linux: systemd service (`/etc/systemd/system/sapo-printer-cert-manager.service`), `User=root`.

Đăng ký trong installer script (chạy 1 lần lúc cài).

#### Nhiệm vụ helper

```rust
// src-tauri/src/bin/sapo_printer_cert_manager.rs
#[tokio::main]
async fn main() -> Result<()> {
    ensure_ca_exists().await?;              // First-run: sinh CA + install trust store
    ensure_server_cert().await?;            // First-run hoặc expired
    let ipc = start_ipc_server().await?;    // Named pipe / unix socket
    let renewal = spawn_renewal_loop();     // Tick 24h
    tokio::select! {
        _ = ipc => {}
        _ = renewal => {}
        _ = shutdown_signal() => {}
    }
    Ok(())
}
```

**Renewal loop**:
```rust
async fn renewal_loop() {
    let mut ticker = tokio::time::interval(Duration::from_secs(24 * 3600));
    loop {
        ticker.tick().await;
        match check_status() {
            RenewalStatus::RenewSoon => renew_server_cert().await.log_err(),
            RenewalStatus::CaRotationNeeded => rotate_ca().await.log_err(),
            _ => {}
        }
    }
}
```

**IPC commands** (JSON line protocol):
```
→ { "cmd": "renew_now" }
← { "ok": true, "expires_at": 1780000000 }

→ { "cmd": "get_status" }
← { "ca_expires_at": ..., "server_expires_at": ..., "ca_trusted": true }

→ { "cmd": "rotate_ca" }
← { "ok": true, "new_ca_pem": "..." }
```

App IPC client dùng khi user click "Renew now" trong Settings hoặc detect cert sắp hết < 7 ngày.

#### Permissions file layout

| File | Path (Windows) | Path (macOS/Linux) | Mode | Owner |
|------|----------------|--------------------|------|-------|
| `ca.key` | `C:\ProgramData\SapoPrinter\tls\ca.key` | `/var/lib/sapo-printer/tls/ca.key` | 0600 | SYSTEM/root |
| `ca.pem` | `C:\ProgramData\SapoPrinter\tls\ca.pem` | `/var/lib/sapo-printer/tls/ca.pem` | 0644 | SYSTEM/root |
| `server.key` | `C:\ProgramData\SapoPrinter\tls\server.key` | `/var/lib/sapo-printer/tls/server.key` | 0640 | SYSTEM/root, group `sapo-printer` |
| `server.pem` | `C:\ProgramData\SapoPrinter\tls\server.pem` | `/var/lib/sapo-printer/tls/server.pem` | 0644 | SYSTEM/root |
| IPC socket | `\\.\pipe\sapo-printer-agent` | `/var/run/sapo-printer-agent.sock` | 0660 | SYSTEM/root, group `sapo-printer` |

App user phải thuộc group `sapo-printer` (installer tạo group + add current user). Windows: ACL cho `Users` read `server.*`.

#### App-side integration

```rust
// src-tauri/src/infrastructure/tls/cert_watcher.rs
pub fn spawn_cert_watcher(server_pem_path: PathBuf, tls_config: Arc<RustlsConfig>) {
    let (tx, rx) = notify::channel();
    let mut watcher = notify::recommended_watcher(tx).unwrap();
    watcher.watch(&server_pem_path, RecursiveMode::NonRecursive).unwrap();
    tauri::async_runtime::spawn(async move {
        for _event in rx {
            if let Err(e) = tls_config.reload_from_pem_file(&server_pem_path, &key_path).await {
                tracing::error!("TLS reload failed: {}", e);
            } else {
                tracing::info!("TLS cert hot-reloaded");
            }
        }
    });
}
```

#### Fallback nếu helper service down

- App detect helper unreachable (IPC connect fail) + cert < 7 ngày → hiển thị notification "Cần chạy công cụ refresh cert" → link mở elevated helper thủ công (UAC prompt).
- Không tự động UAC background — UI rõ ràng.

---

### Component 6: Circuit Breaker Integration (Downloader)

Downloader đã có sẵn `infrastructure/downloader/circuit_breaker.rs`. Wire vào print job handler HTTP:

- `POST /api/v1/jobs` → `CreatePrintJobUseCase` → khi worker download PDF → gọi `ReqwestDownloader` bọc `CircuitBreaker`.
- Nếu circuit open → `InfrastructureError::CircuitOpenError` → job status `FAILED` với `error_code=CIRCUIT_OPEN`, không retry ngay, wait cooldown.

---

### Component 7: Webapp Implementation

#### 7.1 `PrintAgentClient`

```typescript
class PrintAgentClient {
  private port: number | null = Number(localStorage.getItem('sapo_agent_port')) || null;
  private token = localStorage.getItem('sapo_print_token');

  private baseUrl(): string {
    const p = this.port ?? 18901;
    return `https://local.mysapo.net:${p}/api/v1`;
  }

  async discoverPort(): Promise<number | null> {
    for (const port of [18901, 18902, ..., 18910]) {
      try {
        const res = await fetch(`https://local.mysapo.net:${port}/api/v1/ping`, {
          signal: AbortSignal.timeout(200),
        });
        if (res.ok) {
          const { version, min_webapp_version } = await res.json();
          this.checkVersionCompat(min_webapp_version);
          localStorage.setItem('sapo_agent_port', String(port));
          this.port = port;
          return port;
        }
      } catch {}
    }
    return null;
  }

  async isAvailable(): Promise<boolean>;
  async pair(): Promise<{ api_token: string; expires_at: number }>;
  async createJobs(pdfUrls: string[], printerName: string): Promise<string[]>;
  async getJobStatus(jobId: string): Promise<PrintJobStatus>;
  async getPrinters(): Promise<Printer[]>;
}
```

#### 7.2 `usePrintAgent` hook

- Poll `isAvailable()` mỗi 5s nếu chưa kết nối. Backoff exponential nếu fail liên tục.
- SSE reconnect với `Last-Event-ID`:
  ```typescript
  const es = new EventSource(
    `https://local.mysapo.net:${port}/api/v1/events?token=${token}`
  );
  ```
- Fallback WebSocket nếu SSE reconnect fail 3 lần liên tiếp (Safari SSE reconnect có bug).

#### 7.3 `AgentSetupGuide` (khi `agentStatus === 'unavailable'`)

Giữ như thiết kế cũ. Thêm link download OS-specific (detect qua `navigator.userAgent`).

#### 7.4 `PrintStatusModal` — Giữ như cũ.

#### 7.5 Pairing flow — Giữ như cũ, nhưng token response bổ sung `expires_at`.

---

### Component 8: Tích hợp App Startup

#### [MODIFY] `src-tauri/src/main.rs`

```rust
// Trong .setup():

// 1. Cert lifecycle
let cert_bundle = CertGenerator::load_or_generate(&data_dir)?;
if cert_bundle.is_newly_generated {
    CertInstaller::install_ca(&cert_bundle.ca_cert_pem, &data_dir)?;
}
// Background: check renewal 24h/lần
tauri::async_runtime::spawn(cert_renewal_task(data_dir.clone()));

// 2. Token manager
let token_manager = Arc::new(ApiTokenManager::new(pool.clone()));

// 3. SSE broadcaster
let sse_broadcaster = Arc::new(SseBroadcaster::new());
for event_type in JOB_STATUS_EVENTS {
    event_bus.subscribe(event_type, sse_broadcaster.clone());
}

// 4. Bind port với fallback
let (listener, port) = port_binder::bind_with_fallback(18901, 18901..=18910)?;
write_agent_metadata(&data_dir, port)?;

// 5. Start HTTPS server
tauri::async_runtime::spawn(async move {
    HttpServer::start(listener, cert_bundle, HttpServerState {
        job_repo, config_provider, printer_manager,
        event_bus, token_manager, sse_broadcaster,
    }).await.unwrap_or_else(|e| tracing::error!("HTTPS server failed: {}", e));
});
```

#### [MODIFY] `src-tauri/src/lib.rs`

Thêm `token_manager`, `agent_port` vào `AppContextState`.

#### Uninstall hook (Windows MSI / macOS pkg)

Chạy `sapo-printer --uninstall-ca` xóa CA khỏi trust store trước khi xóa binary.

---

## Verification Plan

### Automated Tests

```bash
cargo test -p sapo-printer-pro-max -- tls::
cargo test -p sapo-printer-pro-max -- http_server::
cargo test -p sapo-printer-pro-max -- api_token::   # Timing attack, rate limit
cargo test -p sapo-printer-pro-max -- cert_renewal:: # Expiry path
cargo build --release
```

### Manual Verification

| Scenario | Expected |
|----------|----------|
| Cài lần đầu Windows (non-admin user) | CA vào CurrentUser\Root, Chrome/Edge không warning |
| Cài lần đầu macOS | Dialog xác thực user password, cert vào login.keychain, Safari OK |
| Cài lần đầu Linux | NSS DB update, Chrome/Firefox OK |
| **Safari macOS + iOS** | `https://local.mysapo.net:18901` không mixed-content block |
| Webapp ping | `{ status, version, min_webapp_version, features }` |
| Pairing từ origin `evil.mysapo.net.attacker.com` | CORS blocked (parse Origin) |
| Spam 10 lần `/pair` | Rate limit 429 sau lần 3 |
| Pairing legit | Toast, Allow → token nhận, DB lưu hash không plaintext |
| Send job → SSE | `PENDING → QUEUED → DOWNLOADED → PRINTING → COMPLETED` |
| Restart app | Port ghi ra file, webapp reconnect qua discovery |
| Port 18901 chiếm | Fallback 18902, webapp discover thành công |
| Cert còn 29 ngày | Auto renew, TLS reload không restart |
| Token hết hạn | 401 với `error_code=TOKEN_EXPIRED`, webapp trigger re-pair |
| Uninstall app | CA gỡ khỏi trust store |
| Log SSE request | Token bị mask `?token=***` |

---

## Thứ tự Implementation

```
Phase 0 — PoC Safari + local.mysapo.net (1-2 ngày)     ← THÊM MỚI
├── DNS verify local.mysapo.net → 127.0.0.1
├── Minimal HTTPS server + self-signed CA
├── Test load HTTPS iframe từ https://admin.mysapo.net
├── Test trên Safari macOS, Safari iOS, Chrome, Edge, Firefox
└── Gate: pass hết → tiếp Phase 1. Fail → đổi kiến trúc (native messaging?).

Phase 1 — Cert Infrastructure + Helper Service (6-8 ngày)
├── cert_generator.rs (rcgen, CA 10y + server 397d)
├── cert_installer.rs (Windows LocalMachine, macOS System.keychain, Linux system trust)
├── cert_uninstaller.rs
├── src-tauri/src/bin/sapo_printer_cert_manager.rs (helper service binary)
│   ├── main.rs (service entry, IPC server, renewal loop)
│   ├── Windows Service registration
│   ├── launchd plist + install script
│   └── systemd unit + install script
├── App-side cert_watcher.rs (notify + hot reload rustls)
├── App-side IPC client (renew_now, get_status)
└── Unit + integration tests

Phase 2 — HTTPS Server + Auth (4-5 ngày)
├── Axum + rustls + axum-server + hot reload TLS
├── ApiTokenManager (SHA-256 hash + salt + sliding expiry + constant-time compare)
├── Rate limit tower_governor + per-origin limit
├── REST endpoints + auth middleware
├── CORS URL-parse predicate
├── Port fallback binder
└── Integration tests (rate limit, timing, token expiry)

Phase 3 — SSE (2-3 ngày)
├── SseBroadcaster → EventBus bridge
├── SSE handler + Last-Event-ID replay ring buffer
├── Ping keepalive 15s
├── Log scrubbing (token query mask)
└── End-to-end test

Phase 4 — Tauri Integration + Installer (3-4 ngày)
├── Wiring main.rs setup(), agent.json write
├── Commands: get_paired_origins, revoke_token, get_agent_port, renew_cert_now
├── Desktop UI: pairing toast + danh sách kết nối + revoke + cert status panel
├── Installer scripts (Wix MSI / pkg / deb / rpm):
│   ├── Elevated cert install (LocalMachine / System.keychain / update-ca-certificates)
│   ├── Register helper service (sc / launchctl / systemctl)
│   ├── Create group `sapo-printer` + add current user
│   └── Uninstall hook: xóa CA + gỡ service
└── Circuit breaker wire vào downloader path

Phase 5 — Webapp (5-7 ngày, team frontend)
├── PrintAgentClient + port discovery + version check
├── usePrintAgent hook + SSE reconnect + WS fallback
├── AgentSetupGuide + OS-detect download link
├── PrintStatusModal
└── E2E test trên 4 browser
```

**Tổng ước tính realistic: 22–30 ngày (cả desktop + webapp + helper service + installer)**

---

## Rủi ro chính

| Rủi ro | Impact | Giảm thiểu |
|--------|--------|-----------|
| Safari chặn `https://local.mysapo.net` từ HTTPS parent | Blocker | Phase 0 PoC gate |
| Browser tương lai giảm max leaf cert lifetime < 397d | Cần renew nhanh hơn | Renewal task đã có, chỉ chỉnh threshold |
| DNS `local.mysapo.net` bị hack đổi record | Kết nối sai IP, cert mismatch → fail safe | SAN có `127.0.0.1` + `localhost` fallback |
| Token DB rò | Chỉ hash lộ, plaintext không | Hash SHA-256 + salt |
| Port range 18901-18910 chiếm hết | App không start server | Log rõ, hiện error UI cho user |
| CA private key rò trên 1 máy | Attacker giả cert chỉ cho MÁY ĐÓ (CA per-machine, không cross-trust) | `ca.key` mode 0600 owner SYSTEM/root, chỉ helper service đọc |
| Helper service bị disable/crash | Cert không auto-renew | App detect < 7 ngày → notification + link elevated helper thủ công |
| User không thuộc group `sapo-printer` | App không đọc được `server.key` | Installer add user vào group; runtime check + hướng dẫn re-login |
