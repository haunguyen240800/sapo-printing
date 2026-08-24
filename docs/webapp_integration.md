# Sapo Printer Pro Max — Webapp Integration Guide

Hướng dẫn tích hợp webapp (React/Vue/vanilla JS) với Sapo Printer Pro Max desktop agent để in ấn từ browser.

**Đối tượng**: Frontend engineer làm việc trên `admin.mysapo.net` hoặc subdomain khác của `mysapo.net`.

---

## Mục lục

1. [Tổng quan kiến trúc](#1-tổng-quan-kiến-trúc)
2. [Yêu cầu môi trường](#2-yêu-cầu-môi-trường)
3. [API Reference](#3-api-reference)
4. [Flow tích hợp end-to-end](#4-flow-tích-hợp-end-to-end)
5. [Client implementation](#5-client-implementation-typescript)
6. [React hook example](#6-react-hook-example)
7. [Error handling](#7-error-handling)
8. [Testing](#8-testing)
9. [FAQ](#9-faq)

---

## 1. Tổng quan kiến trúc

```
┌─────────────────────────┐         HTTP         ┌────────────────────────────┐
│  Webapp (browser)       │ ───────────────────► │ Sapo Printer Pro Max       │
│  https://*.mysapo.net   │ ◄─────── SSE ─────── │  http://127.0.0.1          │
└─────────────────────────┘                       │  :18901 (fallback 18902…) │
                                                  └────────────────────────────┘
                                                             │
                                                             ▼
                                                       Windows/macOS/Linux
                                                       Printer System API
```

- Desktop app expose HTTP server chỉ trên IPv4 loopback.
- Webapp gọi REST API tạo print job, subscribe SSE nhận trạng thái real-time.
- Webapp kết nối trực tiếp tới `127.0.0.1`; không cần DNS hay certificate local.

---

## 2. Yêu cầu môi trường

### Browser
- Chrome 90+, Edge 90+, Safari 15+, Firefox 100+.
- **EventSource API** (SSE) native — không cần polyfill.

### Origin cho phép
Chỉ các origin sau được desktop agent chấp nhận (CORS whitelist):
- `https://mysapo.net`
- `https://*.mysapo.net` (bất kỳ subdomain, bao gồm `admin.mysapo.net`)

**Không hoạt động**:
- HTTP (chỉ HTTPS).
- `localhost`, `127.0.0.1` (dùng desktop app nếu dev).
- Domain khác `mysapo.net`.

### Desktop app installed
User phải cài và chạy Sapo Printer Pro Max. Nếu chưa có → hiển thị `AgentSetupGuide` với link tải.

---

## 3. API Reference

Base URL: `http://127.0.0.1:<port>/api/v1` (port từ discovery, mặc định 18901).

### 3.1. `GET /ping`

Health check. **Không cần auth.**

**Request**: không body.

**Response 200**:
```json
{
  "status": "ok",
  "version": "0.1.0",
  "min_webapp_version": "1.0.0",
  "features": ["sse", "pair"],
  "port": 18901
}
```

- `version`: version của desktop app.
- `min_webapp_version`: version webapp tối thiểu cần. Nếu webapp cũ hơn → yêu cầu user update webapp.
- `features`: capabilities bật (`sse`, `pair`, tương lai `websocket`, `printer_config`).

---

### 3.2. `POST /pair` — Tạo token pairing lần đầu

**Không cần auth**. Chỉ gọi 1 lần cho mỗi origin. Rate limit 3 request/giờ/origin.

**Request**:
```json
{ "origin": "https://admin.mysapo.net" }
```

Header **bắt buộc**:
```
Origin: https://admin.mysapo.net
Content-Type: application/json
```

Origin header phải khớp body `origin` — chống CSRF.

**Flow**:
1. Server nhận request.
2. Desktop hiện toast: `"admin.mysapo.net muốn kết nối in ấn"` với [Từ chối] [Cho phép].
3. **Long-polling tối đa 60 giây**:
   - User Allow → server trả 200 với token.
   - User Deny → 403.
   - Không phản hồi 60s → 408.

**Response 200**:
```json
{
  "api_token": "6e1f2c...abcd",
  "expires_at": 1712345678
}
```

- `api_token`: 32 bytes hex (64 ký tự). **Chỉ trả 1 lần lúc này. Lưu ngay vào `localStorage`.**
- `expires_at`: Unix timestamp giây. Token sliding-expire: mỗi lần verify pass → gia hạn +90 ngày. Nếu 90 ngày không dùng → hết hạn, phải re-pair.

**Error codes**:
- `400` — Origin header khớp body.origin fail, hoặc origin không nằm trong whitelist.
- `403` — User denied.
- `408` — User không phản hồi trong 60s.
- `429` — Rate limited (đã request > 3 lần trong 1 giờ).

---

### 3.3. `POST /jobs` — Tạo print job

**Auth**: `Authorization: Bearer <api_token>`.

**Request**:
```json
{
  "printer_name": "HP LaserJet Pro M15",
  "document_urls": [
    "https://s3.sapo.net/orders/123/invoice.pdf",
    "https://s3.sapo.net/orders/124/invoice.pdf"
  ]
}
```

- `printer_name`: tên máy in (lấy từ `GET /printers`).
- `document_urls`: danh sách PDF URL. Tối đa **5000/request**. Auto-batch 50/lần.

**Response 200**:
```json
{
  "job_ids": [
    "3fa85f64-5717-4562-b3fc-2c963f66afa6",
    "4gb85f64-5717-4562-b3fc-2c963f66afa7"
  ]
}
```

Mỗi URL = 1 job. Job vào queue, worker xử lý async.

**Error**:
- `400` — Body invalid, printer_name rỗng, urls > 5000.
- `401` — Token invalid/expired.
- `503` — Printer offline hoặc queue full.

---

### 3.4. `GET /jobs/{id}` — Lấy trạng thái job

**Auth**: Bearer.

**Response 200**:
```json
{
  "id": "3fa85f64-5717-4562-b3fc-2c963f66afa6",
  "printer_name": "HP LaserJet Pro M15",
  "document_url": "https://s3.sapo.net/orders/123/invoice.pdf",
  "status": "COMPLETED",
  "retry_count": 0,
  "created_at": 1712345000,
  "updated_at": 1712345030,
  "completed_at": 1712345030,
  "error_message": null
}
```

**Status enum**:
- `PENDING` — Vừa tạo, chưa vào queue.
- `QUEUED` — Trong queue.
- `DOWNLOADING` — Đang tải PDF từ S3.
- `RENDERING` — Đang render.
- `PRINTING` — Đã gửi tới printer.
- `COMPLETED` — Xong.
- `FAILED` — Lỗi. Xem `error_message`.
- `CANCELLED` — User cancel.

**Error**:
- `404` — Job không tồn tại.
- `401` — Token invalid.

---

### 3.5. `GET /printers` — Danh sách máy in

**Auth**: Bearer.

**Response 200**:
```json
{
  "printers": [
    {
      "name": "HP LaserJet Pro M15",
      "device_id": "hp-laserjet-pro-m15",
      "printer_type": "Local",
      "status": "Online",
      "is_default": true
    },
    {
      "name": "Canon PIXMA G3020",
      "device_id": "canon-pixma-g3020",
      "printer_type": "Local",
      "status": "Offline",
      "is_default": false
    }
  ]
}
```

**Status enum**: `Online`, `Offline`, `Error`, `Unknown`.

---

### 3.6. `GET /events` — SSE stream

**Auth**: `?token=<api_token>` trong query (EventSource không set custom header được).

**Header optional**: `Last-Event-ID: <id>` — resume từ event ID cụ thể sau reconnect. Server replay 100 event gần nhất.

**Event format**:
```
event: PrintJobStarted
id: 42
data: {"job_id":"3fa85f64-...","status":"DOWNLOADING","progress":10}

event: PrintJobCompleted
id: 43
data: {"job_id":"3fa85f64-...","status":"COMPLETED"}
```

**Event types**:
- `PrintJobCreated`
- `PrintJobQueued`
- `PrintJobStarted`
- `PrintJobDownloaded`
- `PrintJobRendered`
- `PrintJobSubmitted`
- `PrintJobCompleted`
- `PrintJobFailed`

**Keepalive**: server gửi `: ping\n\n` mỗi 15s. Client tự động ignore.

**Reconnect**: EventSource native tự reconnect. Server support `Last-Event-ID` để không mất event.

---

## 4. Flow tích hợp end-to-end

### 4.1. Bootstrap sequence

```
┌─────────────────────────────────────────────────────────┐
│ 1. Discover port (loop 18901–18910)                    │
│    ├─ Có port responsive → cache localStorage          │
│    └─ Không có → setAgentStatus('unavailable')         │
│                    hiện AgentSetupGuide                 │
├─────────────────────────────────────────────────────────┤
│ 2. Check ping.min_webapp_version                        │
│    └─ Webapp cũ hơn → hiện update notice               │
├─────────────────────────────────────────────────────────┤
│ 3. Load token từ localStorage                          │
│    ├─ Có token → verify qua GET /printers              │
│    │   ├─ 200 → setAgentStatus('available')            │
│    │   └─ 401 → clear token, goto 4                    │
│    └─ Không có → goto 4                                │
├─────────────────────────────────────────────────────────┤
│ 4. Trigger pairing                                      │
│    ├─ POST /pair với origin                            │
│    ├─ User Allow trên desktop → nhận token → save      │
│    ├─ Deny → hiện "Vui lòng bấm Cho phép trên app"    │
│    └─ Timeout → retry                                   │
├─────────────────────────────────────────────────────────┤
│ 5. Subscribe SSE                                        │
│    └─ EventSource /events?token=...                    │
└─────────────────────────────────────────────────────────┘
```

### 4.2. Print sequence

```
User click "In hàng loạt"
      │
      ▼
webapp: PrintAgentClient.createJobs(printer, urls)
      │  POST /jobs
      ▼
desktop: return { job_ids: [...] }
      │
      ▼
webapp: setJobs(new Map([[id, {status:'PENDING'}], ...]))
      │
      │  SSE stream (đã subscribe từ trước)
      ▼
webapp: es.onmessage → update job.status theo event_type
      │
      ▼
UI PrintStatusModal render progress bar mỗi job
      │
      ▼
COMPLETED / FAILED → toast + remove from active list
```

---

## 5. Client implementation (TypeScript)

### 5.1. `PrintAgentClient`

```typescript
// services/print-agent-client.ts

export const AGENT_HOST = '127.0.0.1';
export const AGENT_PORT_RANGE = [18901, 18902, 18903, 18904, 18905, 18906, 18907, 18908, 18909, 18910] as const;
const PORT_CACHE_KEY = 'sapo_agent_port';
const TOKEN_KEY = 'sapo_print_token';

export interface PingResponse {
  status: 'ok';
  version: string;
  min_webapp_version: string;
  features: string[];
  port: number;
}

export interface Printer {
  name: string;
  device_id: string;
  printer_type: 'Local' | 'Network';
  status: 'Online' | 'Offline' | 'Error' | 'Unknown';
  is_default: boolean;
}

export interface JobStatus {
  id: string;
  status: 'PENDING' | 'QUEUED' | 'DOWNLOADING' | 'RENDERING' | 'PRINTING'
        | 'COMPLETED' | 'FAILED' | 'CANCELLED';
  printer_name: string;
  document_url: string;
  retry_count: number;
  created_at: number;
  updated_at: number;
  completed_at?: number;
  error_message?: string;
}

export class AgentUnavailableError extends Error {}
export class AgentAuthError extends Error {}
export class AgentPairError extends Error {
  constructor(public reason: 'denied' | 'timeout' | 'rate_limited' | 'origin_invalid') {
    super(`Pair failed: ${reason}`);
  }
}

export class PrintAgentClient {
  private port: number | null = null;
  private token: string | null = null;

  constructor() {
    this.port = Number(localStorage.getItem(PORT_CACHE_KEY)) || null;
    this.token = localStorage.getItem(TOKEN_KEY);
  }

  private baseUrl(): string {
    if (!this.port) throw new AgentUnavailableError('Port not discovered');
    return `http://${AGENT_HOST}:${this.port}/api/v1`;
  }

  /** Loop ping 18901-18910, return port đầu tiên OK. Cache localStorage. */
  async discoverPort(): Promise<number | null> {
    const cached = this.port;
    const order = cached
      ? [cached, ...AGENT_PORT_RANGE.filter(p => p !== cached)]
      : [...AGENT_PORT_RANGE];

    for (const port of order) {
      try {
        const ctrl = new AbortController();
        const timer = setTimeout(() => ctrl.abort(), 300);
        const res = await fetch(`http://${AGENT_HOST}:${port}/api/v1/ping`, {
          signal: ctrl.signal,
        });
        clearTimeout(timer);
        if (res.ok) {
          this.port = port;
          localStorage.setItem(PORT_CACHE_KEY, String(port));
          return port;
        }
      } catch {
        // continue
      }
    }
    this.port = null;
    localStorage.removeItem(PORT_CACHE_KEY);
    return null;
  }

  async ping(): Promise<PingResponse> {
    const res = await fetch(`${this.baseUrl()}/ping`);
    if (!res.ok) throw new AgentUnavailableError();
    return res.json();
  }

  /** Trigger pairing. Long-poll ~60s tới khi user resolve. */
  async pair(): Promise<{ api_token: string; expires_at: number }> {
    const origin = window.location.origin;
    const res = await fetch(`${this.baseUrl()}/pair`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ origin }),
    });

    if (res.status === 403) throw new AgentPairError('denied');
    if (res.status === 408) throw new AgentPairError('timeout');
    if (res.status === 429) throw new AgentPairError('rate_limited');
    if (res.status === 400) throw new AgentPairError('origin_invalid');
    if (!res.ok) throw new Error(`Pair failed: ${res.status}`);

    const data = await res.json();
    this.token = data.api_token;
    localStorage.setItem(TOKEN_KEY, data.api_token);
    return data;
  }

  clearToken() {
    this.token = null;
    localStorage.removeItem(TOKEN_KEY);
  }

  hasToken(): boolean {
    return this.token !== null;
  }

  private async authed(path: string, init?: RequestInit): Promise<Response> {
    if (!this.token) throw new AgentAuthError('No token');
    const res = await fetch(`${this.baseUrl()}${path}`, {
      ...init,
      headers: {
        ...(init?.headers ?? {}),
        Authorization: `Bearer ${this.token}`,
      },
    });
    if (res.status === 401) {
      this.clearToken();
      throw new AgentAuthError('Token expired');
    }
    return res;
  }

  async createJobs(printerName: string, documentUrls: string[]): Promise<string[]> {
    const res = await this.authed('/jobs', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ printer_name: printerName, document_urls: documentUrls }),
    });
    if (!res.ok) throw new Error(`Create jobs failed: ${res.status}`);
    const data = await res.json();
    return data.job_ids;
  }

  async getJob(id: string): Promise<JobStatus> {
    const res = await this.authed(`/jobs/${id}`);
    if (!res.ok) throw new Error(`Get job failed: ${res.status}`);
    return res.json();
  }

  async getPrinters(): Promise<Printer[]> {
    const res = await this.authed('/printers');
    if (!res.ok) throw new Error(`Get printers failed: ${res.status}`);
    const data = await res.json();
    return data.printers;
  }

  /** Open SSE stream. Caller quản lý lifecycle. */
  openEventStream(onEvent: (type: string, data: any) => void, onError?: (e: Event) => void): EventSource {
    if (!this.token) throw new AgentAuthError('No token');
    const url = `${this.baseUrl()}/events?token=${encodeURIComponent(this.token)}`;
    const es = new EventSource(url);
    const jobEvents = [
      'PrintJobCreated', 'PrintJobQueued', 'PrintJobStarted',
      'PrintJobDownloaded', 'PrintJobRendered', 'PrintJobSubmitted',
      'PrintJobCompleted', 'PrintJobFailed',
    ];
    for (const type of jobEvents) {
      es.addEventListener(type, (e: MessageEvent) => {
        try {
          onEvent(type, JSON.parse(e.data));
        } catch {
          // ignore malformed
        }
      });
    }
    if (onError) es.onerror = onError;
    return es;
  }
}

export const printAgent = new PrintAgentClient();
```

### 5.2. Version compat helper

```typescript
// utils/version.ts
export function isVersionAtLeast(current: string, required: string): boolean {
  const c = current.split('.').map(Number);
  const r = required.split('.').map(Number);
  for (let i = 0; i < Math.max(c.length, r.length); i++) {
    const a = c[i] ?? 0, b = r[i] ?? 0;
    if (a > b) return true;
    if (a < b) return false;
  }
  return true;
}
```

---

## 6. React hook example

```typescript
// hooks/use-print-agent.ts
import { useEffect, useReducer, useRef, useCallback } from 'react';
import { printAgent, AgentPairError, AgentUnavailableError, type JobStatus } from '../services/print-agent-client';

type AgentStatus =
  | { kind: 'checking' }
  | { kind: 'unavailable' }
  | { kind: 'update_required'; min: string; current: string }
  | { kind: 'pairing' }
  | { kind: 'available' }
  | { kind: 'pair_denied' }
  | { kind: 'error'; message: string };

interface State {
  status: AgentStatus;
  jobs: Map<string, Partial<JobStatus>>;
}

type Action =
  | { type: 'setStatus'; status: AgentStatus }
  | { type: 'updateJob'; id: string; data: Partial<JobStatus> }
  | { type: 'resetJobs' };

function reducer(state: State, action: Action): State {
  switch (action.type) {
    case 'setStatus':
      return { ...state, status: action.status };
    case 'updateJob': {
      const jobs = new Map(state.jobs);
      jobs.set(action.id, { ...jobs.get(action.id), ...action.data });
      return { ...state, jobs };
    }
    case 'resetJobs':
      return { ...state, jobs: new Map() };
  }
}

export function usePrintAgent() {
  const [state, dispatch] = useReducer(reducer, {
    status: { kind: 'checking' },
    jobs: new Map(),
  });
  const esRef = useRef<EventSource | null>(null);

  const connect = useCallback(async () => {
    dispatch({ type: 'setStatus', status: { kind: 'checking' } });

    const port = await printAgent.discoverPort();
    if (!port) {
      dispatch({ type: 'setStatus', status: { kind: 'unavailable' } });
      return;
    }

    // Version check
    try {
      const info = await printAgent.ping();
      // TODO: compare WEBAPP_VERSION vs info.min_webapp_version
    } catch {
      dispatch({ type: 'setStatus', status: { kind: 'unavailable' } });
      return;
    }

    if (!printAgent.hasToken()) {
      dispatch({ type: 'setStatus', status: { kind: 'pairing' } });
      try {
        await printAgent.pair();
      } catch (e) {
        if (e instanceof AgentPairError && e.reason === 'denied') {
          dispatch({ type: 'setStatus', status: { kind: 'pair_denied' } });
        } else {
          dispatch({ type: 'setStatus', status: { kind: 'error', message: String(e) } });
        }
        return;
      }
    }

    // Verify token bằng GET /printers
    try {
      await printAgent.getPrinters();
    } catch {
      printAgent.clearToken();
      dispatch({ type: 'setStatus', status: { kind: 'pairing' } });
      return;
    }

    // Subscribe SSE
    esRef.current?.close();
    esRef.current = printAgent.openEventStream(
      (type, data) => {
        if (data.job_id) {
          dispatch({ type: 'updateJob', id: data.job_id, data });
        }
      },
      () => {
        // EventSource native tự reconnect. Nếu vẫn fail sau nhiều lần → mark error.
      }
    );

    dispatch({ type: 'setStatus', status: { kind: 'available' } });
  }, []);

  useEffect(() => {
    connect();
    // Poll khi unavailable
    const timer = setInterval(() => {
      if (state.status.kind === 'unavailable' || state.status.kind === 'pair_denied') {
        connect();
      }
    }, 5000);
    return () => {
      clearInterval(timer);
      esRef.current?.close();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const print = useCallback(
    async (printerName: string, urls: string[]) => {
      const ids = await printAgent.createJobs(printerName, urls);
      ids.forEach(id =>
        dispatch({ type: 'updateJob', id, data: { id, status: 'PENDING' } })
      );
      return ids;
    },
    []
  );

  return {
    status: state.status,
    jobs: state.jobs,
    print,
    reconnect: connect,
  };
}
```

### Usage trong component

```typescript
function BulkPrintButton({ orders }: { orders: Order[] }) {
  const { status, jobs, print } = usePrintAgent();

  if (status.kind === 'checking') return <Spinner />;
  if (status.kind === 'unavailable') return <AgentSetupGuide />;
  if (status.kind === 'pairing') return <div>Chờ user xác nhận trên desktop app...</div>;
  if (status.kind === 'pair_denied') return <div>Vui lòng Cho phép trên Sapo Printer Pro Max</div>;

  const handleClick = async () => {
    const urls = orders.map(o => o.invoicePdfUrl);
    await print('HP LaserJet', urls);
  };

  return (
    <>
      <button onClick={handleClick} disabled={status.kind !== 'available'}>
        In {orders.length} phiếu
      </button>
      <PrintStatusModal jobs={jobs} />
    </>
  );
}
```

---

## 7. Error handling

| Tình huống | Detect | Xử lý |
|-----------|--------|-------|
| Desktop chưa cài / không chạy | `discoverPort()` return null | Hiện `AgentSetupGuide` với link tải |
| Token expired (90 ngày không dùng) | Bất kỳ authed request → 401 | Clear token, trigger pair lại |
| User deny pair | `POST /pair` → 403 | Hiển thị "Vui lòng bấm Cho phép" |
| User không phản hồi 60s | 408 | Retry với back-off |
| Rate limit pair | 429 | Wait 1 giờ hoặc yêu cầu user restart app |
| Printer offline | `POST /jobs` → 503 | Show list printers, yêu cầu chọn printer online |
| SSE mất kết nối | `es.onerror` | EventSource tự reconnect. Sau 3 lần fail → force reconnect qua `discoverPort()` |
| Webapp version cũ | `ping.min_webapp_version > current` | Hiện banner "Cần refresh webapp" |
| Desktop app version cũ | `ping.version` cũ hơn expected | Hiện notice "Cập nhật Sapo Printer Pro Max để dùng feature X" |

### Reconnect strategy

```typescript
let reconnectAttempts = 0;
es.onerror = () => {
  reconnectAttempts++;
  if (reconnectAttempts > 3) {
    es.close();
    // Full re-discover
    setTimeout(() => connect(), 3000);
  }
  // < 3 → để EventSource native tự retry
};
es.onopen = () => { reconnectAttempts = 0; };
```

---

## 8. Testing

### 8.1. Manual test checklist

- [ ] `curl http://127.0.0.1:18901/api/v1/ping` → 200
- [ ] Chrome DevTools: fetch `/ping` từ tab `https://admin.mysapo.net` không có CORS error
- [ ] `POST /pair` → toast xuất hiện trên desktop → Allow → token nhận
- [ ] Token lưu localStorage
- [ ] `POST /jobs` → job_ids trả về
- [ ] Job printed thật trên máy in
- [ ] SSE nhận events real-time
- [ ] Reload page → token còn hiệu lực, tự động re-connect
- [ ] Restart desktop app → webapp auto-reconnect qua port discovery
- [ ] Deny pair → hiện thông báo đúng
- [ ] Origin từ `evil.com` → CORS blocked

### 8.2. Automated (mock server)

Dùng MSW hoặc mock server tại `http://127.0.0.1:18901`:

```typescript
// vitest.setup.ts
import { setupServer } from 'msw/node';
import { http, HttpResponse } from 'msw';

export const server = setupServer(
  http.get('http://127.0.0.1:18901/api/v1/ping', () =>
    HttpResponse.json({ status: 'ok', version: '0.1.0', min_webapp_version: '1.0.0', features: ['sse', 'pair'], port: 18901 })
  ),
  http.post('http://127.0.0.1:18901/api/v1/pair', () =>
    HttpResponse.json({ api_token: 'test-token', expires_at: 9999999999 })
  ),
  http.post('http://127.0.0.1:18901/api/v1/jobs', () =>
    HttpResponse.json({ job_ids: ['test-job-1'] })
  ),
);
```

### 8.3. E2E test (Playwright)

Cần desktop app chạy sẵn với `--dev-token` mode bypass pairing:
```bash
sapo-printer --dev-token=fixed-test-token
```

Playwright test:
```typescript
test('bulk print end-to-end', async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.setItem('sapo_print_token', 'fixed-test-token');
    localStorage.setItem('sapo_agent_port', '18901');
  });
  await page.goto('https://admin.mysapo.net/orders');
  await page.click('button:has-text("In hàng loạt")');
  await expect(page.getByText('COMPLETED')).toBeVisible({ timeout: 30000 });
});
```

---

## 9. FAQ

**Q: Vì sao dùng `127.0.0.1` thay vì hostname?**
A: Địa chỉ IPv4 loopback bảo đảm server không expose ra LAN và không phụ thuộc DNS hay trust store.

**Q: Token có expire không?**
A: Có. Sliding expiry 90 ngày. Mỗi lần verify pass thì gia hạn thêm 90 ngày kể từ thời điểm đó. Nếu 90 ngày không hoạt động → hết hạn.

**Q: Có bao nhiêu token cho 1 user?**
A: 1 token per origin per browser. User dùng 2 browser (Chrome + Safari) → cần pair 2 lần.

**Q: Token có thể sync giữa các thiết bị không?**
A: Không. Mỗi máy tính = mỗi desktop app instance = mỗi bộ token riêng.

**Q: Nếu user reset browser (clear localStorage) thì sao?**
A: Phải pair lại. Desktop UI có nút "Xóa tất cả kết nối" để user chủ động revoke.

**Q: Print job có persist khi restart app không?**
A: Có. Job store trong SQLite. Restart app → worker resume xử lý job status `QUEUED` / `DOWNLOADING`.

**Q: Có support in ZPL, image không?**
A: Có. Field `document_url` có thể trỏ tới file ZPL, PNG, JPG. Desktop auto-detect qua content-type.

**Q: Max size PDF?**
A: Không giới hạn cứng. Download timeout 60s. PDF > 100MB có thể fail — nên chia nhỏ.

**Q: Có thể set paper size, orientation qua API không?**
A: Chưa. Hiện dùng config từ desktop app (`printer_configs` table). Roadmap cho phép override qua job payload.

**Q: Nếu printer name có Unicode/dấu tiếng Việt?**
A: OK. Encode UTF-8 trong JSON. Server so exact match.

**Q: Rate limit `/jobs`?**
A: Không giới hạn số request, nhưng mỗi request tối đa 5000 URLs. Auto-batch 50 URLs/lần trong queue.

**Q: Có health endpoint để monitor?**
A: `/ping` không cần auth, có thể poll đều để check aliveness. `/api/v1/metrics` cho paired origins (Sprint 8 roadmap).

**Q: CORS preflight?**
A: Server support `OPTIONS`. Cache-Control 3600s. Chrome cache preflight → chỉ gọi 1 lần/giờ.

---

## Roadmap

Feature sẽ có trong version tương lai (check `ping.features`):

- `websocket` — thay thế SSE trên Safari (fix reconnect bug).
- `printer_config` — override paper/orientation qua job payload.
- `job_batch` — group jobs vào batch, hủy cả batch 1 lần.
- `preview` — sinh thumbnail trước khi in.

---

## Liên hệ

- Bug report: internal Slack channel `#sapo-printer-support`.
- Feature request: JIRA project `SAPRT`.
- Desktop app source: `github.com/sapo/sapo-printing`.
