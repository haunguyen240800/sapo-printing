---
title: "SAPO Printer — Product Requirements Document"
status: final
created: 2026-06-22
updated: 2026-06-22
project: sapo-printer
version: 1.0
---

# SAPO Printer — Product Requirements Document

**Dự án:** SAPO Printer Desktop Application  
**Phiên bản:** v1.0  
**Ngày tạo:** 2026-06-22  
**Tác giả:** dev (BMad Framework)  
**Đối tượng:** Team Dev, PM, Designer, QA  

---

## 1. Product Vision

### Vấn đề

Các doanh nghiệp bán hàng trên SAPO hiện phải in phiếu giao hàng thủ công từng đơn một, hoặc dùng giải pháp của đối thủ với trải nghiệm kém và thiếu kiểm soát. Khi xử lý hàng trăm đơn mỗi ngày, quy trình này tốn thời gian, dễ sai sót, và không scale.

**Thực tế:**
- 90% khách hàng SAPO cần in > 50 đơn/ngày
- 50% khách hàng (macOS + Linux users) **không có giải pháp nào** — các tool hiện tại chỉ support Windows
- Khách hàng đang complain về việc thiếu tool printing hiệu quả

### Giải pháp

SAPO Printer là desktop application cho phép in hàng loạt phiếu giao hàng với một click, tự động xử lý từ tải xuống đến in ấn, retry khi lỗi, và cập nhật trạng thái real-time. Được thiết kế cho nhân viên kho cần xử lý 100-5000 đơn/ngày.

### Tại sao cần

- **Tiết kiệm thời gian:** In 500 đơn trong 10 phút thay vì 2 giờ in thủ công
- **Giảm sai sót:** Tự động hóa toàn bộ quy trình, loại bỏ lỗi thủ công
- **Scale:** Xử lý được peak season (5000 đơn/ngày) mà không cần tăng nhân sự
- **Cạnh tranh:** Giải pháp native của SAPO, tích hợp sâu hơn đối thủ
- **Market coverage:** Phục vụ 100% customer base (Win + macOS + Linux) thay vì chỉ 50%

### Thành công khi

- Nhân viên kho in xong 500 đơn trong < 10 phút với tỷ lệ thành công ≥ 99%
- 60% khách hàng SAPO (có nhu cầu in > 50 đơn/ngày) sử dụng SAPO Printer trong 6 tháng
- Khách hàng SAPO không còn phải dùng tool của đối thủ
- SAPO tăng customer retention nhờ workflow printing tốt hơn
- Giảm 50% complaints về printing workflow

---

## 2. Market Context

### Current State

90% khách hàng SAPO cần in > 50 đơn hàng/ngày, nhưng hiện đang:
- **In thủ công:** Mở từng đơn, click in — không scale, dễ sai sót
- **Dùng third-party tools:** Chỉ hỗ trợ Windows (50% user base)
- **Complain:** Khách hàng đang phản ánh thiếu giải pháp printing hiệu quả

### The Gap

- **50% khách hàng bị bỏ rơi:** macOS (30%) và Linux (20%) users không có tool nào
- **Thiếu transparency:** Không biết job đang ở đâu, lỗi gì, bao lâu nữa xong
- **Không có automation:** Phải retry thủ công khi lỗi, không có batch processing

### SAPO Printer Strategic Value

1. **Market coverage:** Phục vụ 100% customer base (Win + macOS + Linux) thay vì chỉ 50%
2. **Native integration:** First-party solution tích hợp sâu với SAPO, không phụ thuộc third-party
3. **Operational transparency:** Real-time dashboard với metrics đầy đủ
4. **Production-ready:** Xử lý 100-5000 đơn/batch với auto-retry, audit trail

### Success Impact

- Giảm 50% complaints về printing workflow
- Tăng customer satisfaction của macOS/Linux users (từ 0 → có solution)
- Giảm dependency vào third-party tools
- 70% khách hàng chuyển từ third-party tools sang SAPO Printer

---

## 3. Target Users & Core Capabilities

### Target Users

**Primary User: Nhân viên kho/vận hành**
- Xử lý 100-5000 đơn hàng mỗi ngày
- Cần in phiếu giao hàng nhanh, chính xác
- Làm việc trên Windows (50%), macOS (30%), hoặc Linux (20%)
- Ít training về công nghệ — cần UI đơn giản, rõ ràng

**Secondary User: Quản lý kho**
- Giám sát tiến độ in ấn
- Xem báo cáo, metrics
- Troubleshoot khi có vấn đề

**Non-user (nhưng ảnh hưởng): IT/DevOps**
- Deploy và maintain app
- Cấu hình máy in, network
- Troubleshoot technical issues

### Core Capabilities

**C1. Bulk Print Management**
- Nhận lệnh in từ web cho 100-5000 đơn hàng
- Tự động chia batch (50 đơn/batch, configurable)
- Xử lý song song: download, render, print
- Auto-retry khi lỗi (max 3 lần)

**C2. Cross-Platform Printer Support**
- Discover printers trên Windows (Win32 API), macOS/Linux (CUPS)
- Hiển thị trạng thái máy in real-time (online/offline)
- Cấu hình printer: paper size, margins, color mode
- Hybrid printing strategy: Direct PDF (fast) hoặc Render (control)

**C3. Real-Time Status Transparency**
- **Print Status Dashboard** hiển thị:
  - Thông tin cấu hình đã chọn (printer, paper, margins, color)
  - Tiến trình download (X/Y documents downloaded)
  - Tiến trình print (X/Y jobs printed)
  - Thời gian in (elapsed + estimated remaining)
  - Tổng jobs / Thành công / Thất bại
- Status updates về web app (polling v1, WebSocket v2)

**C4. Configuration Management**
- Lưu cấu hình per printer
- Auto-update khi có version mới

**C5. Audit & Observability**
- Log mọi print job với audit trail
- Error reporting chi tiết
- Metrics: job count, duration, success rate

---
## 4. Functional Requirements

### FR-1: Bulk Print Management

**FR-1.1: Nhận Print Request từ Web App**
- System nhận print command qua Native Messaging
- Request gồm: `pdf_urls[]` (S3 links), `printer_name`, `config` (paper, margins, color)
- Validate: số lượng (1-5000), URLs hợp lệ, printer online
- Trả về `job_id` hoặc error

**FR-1.2: Tạo và Quản lý Print Jobs**

**Job States:**
```
PENDING → QUEUED → DOWNLOADED → SUBMITTED_TO_QUEUE → PRINTING → COMPLETED

              ↓ (on error at any step)
            FAILED → (if retry_count < 3) → QUEUED
```

**State definitions:**
- `PENDING` — Job mới tạo
- `QUEUED` — Trong queue, chờ xử lý
- `DOWNLOADED` — PDF đã download, lưu temp file
- `SUBMITTED_TO_QUEUE` — Đã đẩy vào print queue của máy in
- `PRINTING` — Máy in đang xử lý
- `COMPLETED` — Hoàn thành, temp file đã xóa
- `FAILED` — Thất bại (retry nếu < 3 lần)

**FR-1.3: Batch Processing & Queue**
- Khi ≥100 URLs → bật chế độ "In số lượng lớn"
- **Persist:** Lưu tất cả jobs vào database (SQLite)
- **Queue:** Jobs vào durable queue, xử lý tuần tự
- **Download batch:** Download song song (max 10 concurrent), lưu temp files
- **Print sequential:** Xử lý print tuần tự từ queue
- **Cleanup:** Xóa temp file sau khi `COMPLETED` hoặc `FAILED` (sau retry cuối)

**FR-1.4: Auto-Retry Logic**
- Khi job `FAILED`, kiểm tra retry_count < 3
- Nếu có thể retry: đẩy lại vào `QUEUED`, exponential backoff (5s → 10s → 20s)
- Retry cho: download timeout, render error, printer temporary error
- Không retry: validation errors, 404 URLs
- Sau 3 lần → giữ `FAILED` permanently

**FR-1.5: Job Cancellation**
- Cancel được ở: `PENDING`, `QUEUED`, `DOWNLOADED`, `SUBMITTED_TO_QUEUE`
- `PRINTING` có thể cancel với warning
- Không cancel: `COMPLETED`, `FAILED`
- Cleanup temp file khi cancel

---

### FR-2: Printer Management

**FR-2.1: Printer Discovery (Cross-Platform)**
- Tự động phát hiện tất cả máy in khả dụng
- **Windows:** Win32 EnumPrinters API
- **macOS/Linux:** CUPS (lpstat hoặc CUPS API)
- Return danh sách máy in với: tên, trạng thái, loại
- Nếu không có máy in → return empty list

**FR-2.2: Printer Status Monitoring**
- Kiểm tra trạng thái: `ONLINE`, `OFFLINE`, `ERROR`
- Cập nhật định kỳ (mỗi 5 giây khi app active)
- Hiển thị thông báo khi máy in offline/error

**FR-2.3: Printer Configuration**

**Basic Settings:**
- Chọn máy in: Dropdown list
- Paper Size: A4, A5, Letter, Custom
- Custom dimensions: Width × Height (mm)
- In chiều ngang: Checkbox

**Print Mode:**
- In ảnh: Checkbox
- Loại ảnh in: RGB/ARGB/BGR/GRAY/BINARY (hiện khi "In ảnh" ON)

**Layout:**
- Margins: Left, Right, Top, Bottom (mm)

**Advanced:**
- Bật Printing Buffer: Checkbox (default OFF)
- Buffer Size: KB (hiện khi buffer được bật)

**FR-2.4: Default Printer Selection**
- First launch → tự động lấy máy in mặc định từ OS
- Empty list → UI hiển thị cảnh báo

**FR-2.5: Printer Capability Detection**
- Detect Direct PDF support
- Chọn strategy: Direct PDF (fast) hoặc Render (control)

---

### FR-3: Document Processing

**FR-3.1: Document Download**
- Download PDF từ S3 URLs
- Timeout: 30 giây
- Lưu temp directory
- Validate PDF header

**FR-3.2: PDF Rendering Strategy (Hybrid)**
- **Direct PDF:** ~0.5s (fast)
- **Render:** ~2-3s (control)
- Tự động chọn strategy

**FR-3.3: Color Mode Conversion**
Hỗ trợ 5 modes:
- RGB — 24-bit
- ARGB — 32-bit với alpha
- BGR — Windows default
- GRAY — 8-bit grayscale
- BINARY — 1-bit monochrome

**FR-3.4: Margins Application**
- Apply margins (mm) khi render
- Convert mm → pixels at 300 DPI

**FR-3.5: Paper Size Handling**
- Predefined: A4, A5, Letter
- Custom: 50-500mm
- Internal: mm

---

### FR-4: User Interface

**FR-4.1: Print Status Dashboard**
- Thông tin cấu hình: printer, paper, margins, color
- Tiến trình: download, print progress
- Thời gian: elapsed + estimated
- Metrics: tổng/thành công/thất bại
- Job list với filters

**FR-4.2: Printer Configuration Screen**
5 sections: Printer Selection, Paper Settings, Print Mode, Layout, Advanced

**FR-4.3: Auto-Update Popup**
- Auto check khi startup
- Hiển thị: app name, version, website
- Actions: Cập nhật, Đóng, Kiểm tra phiên bản

**FR-4.4: Error Notification & Recovery**
- Toast notifications
- Error detail modal với retry button

---

### FR-5: System Integration

**FR-5.1: Native Messaging**
- Desktop đăng ký host với browser
- JSON protocol: ping, print_batch, get_status, cancel_job, list_printers

**FR-5.2: Status Sync**
- v1: Polling mỗi 2s
- v2: WebSocket real-time

**FR-5.3: Configuration Persistence**
- SQLite: printer_configs, print_jobs, app_settings
- Backup trước update

**FR-5.4: Auto-Update**
- Check: startup + 24h
- Tauri Updater plugin
- Platform: NSIS/DMG/AppImage

---

### FR-6: Audit & Logging

**FR-6.1: Audit Trail**
- Log tất cả jobs với full context
- Retention: 30 ngày

**FR-6.2: Error Logging**
- Structured logs: DEBUG/INFO/WARN/ERROR
- Rotation: daily, 7 ngày

**FR-6.3: Metrics Collection**
- Job metrics, queue metrics, printer metrics, performance
- Export qua UI
## 5. Non-Functional Requirements

### NFR-1: Performance

**Throughput:**
- Xử lý tối thiểu **100 đơn/phút** (trung bình)
- Batch 5000 đơn hoàn thành trong **< 60 phút**

**Latency:**
- Printer discovery: < 2s
- PDF download: < 30s per document
- PDF rendering: < 1s per page (300 DPI, A4)
- Direct PDF print: ~0.5s per job
- Render strategy print: ~2-3s per job
- UI response time: < 200ms

**Resource Usage:**
- Memory: < 500MB khi xử lý 5000 đơn
- CPU: < 70% during peak load
- Disk: Cleanup temp files ngay sau job complete
- Concurrent downloads: Max 10 parallel
- Concurrent renders: Max 10 parallel

**App Startup:**
- Cold start: < 3s từ launch đến UI ready

---

### NFR-2: Reliability & Availability

**Success Rate:**
- Print job success rate: ≥ 99%
- Auto-retry success: ≥ 80% các job lỗi tạm thời được recover

**Uptime:**
- App chạy liên tục 24/7 không crash
- Graceful degradation khi mất network

**Data Durability:**
- Jobs persist qua app restart (durable queue)
- Config backup trước khi update
- Audit trail không bị mất

**Error Recovery:**
- Auto-retry với exponential backoff
- Không crash khi gặp corrupt PDF
- Timeout protection cho tất cả network calls

---

### NFR-3: Usability

**Ease of Use:**
- Người dùng mới sử dụng được trong < 5 phút
- UI tiếng Việt, rõ ràng, trực quan

**Feedback & Transparency:**
- Real-time status updates (mỗi 2s)
- Progress indicators cho download/print
- Error messages rõ ràng, actionable

**Accessibility:**
- Keyboard navigation support
- High contrast mode (optional v2)

---

## 6. Success Metrics

### Product Adoption Metrics

**Target:** 
- **60% khách hàng SAPO** (có nhu cầu in > 50 đơn/ngày) sử dụng SAPO Printer trong 6 tháng sau launch

**Leading Indicators:**
- Downloads per week
- Active installs (cross-platform split: Win/Mac/Linux)
- DAU (Daily Active Users)

---

### Usage Metrics

**Primary:**
- **Số lượng đơn in mỗi ngày:** Target ≥ 50,000 đơn/ngày across all users
- **Số batch jobs:** Target ≥ 500 batch jobs/ngày
- **Avg batch size:** Track distribution (100-500, 500-1000, 1000-5000 đơn)

**Engagement:**
- Print frequency: Số ngày active/tháng per user
- Retention: % users còn active sau 30/60/90 ngày

---

### Quality Metrics

**Success Rate:**
- **Print job success rate:** Target ≥ 99%
- Auto-retry success rate: Target ≥ 80%
- Crash-free sessions: Target ≥ 99.9%

**Performance:**
- P50/P95/P99 print duration (per job)
- P50/P95 batch completion time
- Download failure rate: Target < 1%
- Render failure rate: Target < 0.5%

---

### Business Impact Metrics

**Customer Satisfaction:**
- NPS (Net Promoter Score): Target ≥ 40
- Support tickets về printing: Giảm 50% so với trước khi có SAPO Printer
- Customer complaints: Giảm 50%

**Competitive:**
- % khách hàng ngừng dùng third-party tools: Target ≥ 70%
- macOS/Linux market coverage: 50% users previously không có solution

---

### Counter-Metrics

**Rủi ro cần monitor:**
- **Support burden tăng:** Số support tickets về SAPO Printer
- **Printer compatibility issues:** % printers không work
- **Network dependency:** % jobs failed do S3 timeout

---

## 7. Technical Constraints

### Platform Requirements

**Desktop Platforms:**
- Windows 10/11 (64-bit)
- macOS 10.15+ (Catalina and later)
- Linux (Ubuntu 20.04+, Debian-based distros)

**Browser Support:**
- Google Chrome 90+ (Native Messaging)
- Edge Chromium 90+

---

### Architecture Decisions

**Architectural Pattern:**
- **Clean Architecture + Domain-Driven Design (DDD)**
- 4 layers: Interface → Application → Domain → Infrastructure
- Domain layer độc lập hoàn toàn

**Key Patterns:**
- Repository Pattern — Data access abstraction
- Strategy Pattern — Printer discovery, rendering
- Event-Driven — Domain events
- Use Case Pattern — Application orchestration

---

### Technology Stack

**Framework & Language:**
- **Tauri v2** — Cross-platform desktop framework
- **Rust** — Backend (performance, safety, cross-platform)
- **React 18 + TypeScript** — Frontend UI
- **Vite 7+** — Build tool
- **pnpm** — Package manager

**UI Libraries:**
- **@sapo/ui-components** (^2.19.0) — SAPO design system
- **@sapo/ui-icons** (^1.19.0) — Icon library
- **@emotion/react** (^11.14.0) + **@emotion/styled** (^11.14.1) — CSS-in-JS

**Form & Validation:**
- **react-hook-form** (^7.79.0) — Form state
- **yup** (^1.7.1) — Schema validation
- **@hookform/resolvers** (^5.4.0) — Integration

**State Management:**
- **React Context** — Global app state

**Core Libraries:**
- **MuPDF** — PDF rendering
- **SQLite** — Embedded database
- **Win32 API** (Windows) / **CUPS** (macOS/Linux) — Printer control
- **Tauri Updater Plugin** — Auto-update

---

### Rendering Strategy

**Hybrid Approach:**
- **Direct PDF:** ~0.5s per job (fast)
- **MuPDF Render:** ~2-3s per job (control)

---

### Data Storage

- Config & jobs: SQLite (`~/.sapo-printer/config.db`)
- Temp files: `~/.sapo-printer/temp/` (auto-cleanup)
- Logs: `~/.sapo-printer/logs/` (7 days retention)

---

### Network & Communication

**v1:**
- Native Messaging (Web ↔ Desktop)
- Status polling (mỗi 2s)
- HTTPS downloads từ S3

**v2 (Future):**
- WebSocket real-time sync

---

## 8. Out of Scope

### Deferred to v2 (Future Releases)

**Real-time WebSocket Sync:**
- v1 dùng polling (mỗi 2s)
- v2 sẽ implement WebSocket để push status real-time

**Offline Printing:**
- v1 yêu cầu internet connection để download PDFs từ S3
- v2 có thể support server push jobs để in khi web app offline

**Multi-device Sync:**
- v1 không sync config giữa nhiều devices
- v2 có thể sync config qua cloud

**Advanced Reporting & Analytics:**
- v1 có basic metrics trong UI
- v2 có thể có dashboard tổng hợp, export reports, trends analysis

**Printer Capability Auto-detection:**
- v1 detect cơ bản (PDF support)
- v2 có thể detect chi tiết hơn (color support, paper sizes, duplex)

**Firefox Native Messaging:**
- v1 chỉ support Chrome/Edge
- v2 có thể extend sang Firefox

---

### Explicitly Out of Scope

**Document Generation:**
- SAPO Printer **KHÔNG** generate PDFs
- Web app chịu trách nhiệm generate và upload S3
- Desktop app chỉ download và print

**Document Formats khác PDF:**
- v1 chỉ support PDF
- Không support: ZPL, RAW, ESC/POS, image-only formats

**Network Printer Management:**
- Không add/remove network printers
- Chỉ detect printers đã được OS/user add sẵn

**Print Job Scheduling:**
- Không support "schedule in lúc X giờ"
- Jobs được xử lý ngay khi nhận

**Multi-tenant / Account Management:**
- Không có user login/authentication trong desktop app
- Device token chỉ để identify device

**Cloud Print Services:**
- Không integrate với Google Cloud Print, Apple AirPrint remote printing
- Chỉ in tới máy in local/network đã kết nối

---

## Appendix

### Related Documents

- `docs/srs-in.md` — Architecture specification (DDD + Clean Architecture)
- `docs/SRS_In số lượng lớn.md` — Detailed SRS for bulk printing
- `_bmad-output/brainstorming/cross-platform-printer-architecture-2026-06-22.md` — Technical brainstorming session

### Glossary

- **Print Job:** Một tác vụ in cho một document (PDF)
- **Batch:** Nhóm print jobs được xử lý cùng lúc
- **Queue:** Hàng đợi chứa jobs chờ xử lý
- **Direct PDF:** Strategy in PDF trực tiếp không qua render
- **Render Strategy:** Strategy render PDF thành image trước khi in
- **Hybrid Strategy:** Tự động chọn Direct PDF hoặc Render tùy context
- **Native Messaging:** Protocol giao tiếp giữa browser extension và desktop app

---

**End of Document**
