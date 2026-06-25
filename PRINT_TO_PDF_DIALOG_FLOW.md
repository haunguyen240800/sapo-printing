# Print to PDF Dialog Flow

## Problem
Khi print to PDF, Windows không tự động show "Save As" dialog vì:
- Backend chạy trong background worker thread (không có UI context)
- `pOutputFile` được set = fixed path → Windows skip dialog

## Solution Flow

### Option 1: Frontend-driven (Recommended)
```
1. User clicks "Print" → Frontend check printer name
2. If "Print to PDF" → Frontend show Tauri save dialog FIRST
3. User chọn path (hoặc cancel)
4. Frontend gọi create_print_job với output_path
5. Backend lưu file vào path đã chọn
```

**Pros:**
- UI responsive (dialog ngay lập tức)
- User có thể cancel trước khi tạo job
- Không cần thay đổi backend

**Implementation:**
```typescript
async function handlePrint() {
  if (printerName.includes('Print to PDF')) {
    const path = await save({
      defaultPath: `SAPO_Print_${Date.now()}.pdf`,
      filters: [{ name: 'PDF', extensions: ['pdf'] }]
    });

    if (!path) return; // User cancelled

    await invoke('create_print_job', {
      payload: { pdf_urls, printer_name: printerName, output_path: path }
    });
  } else {
    await invoke('create_print_job', {
      payload: { pdf_urls, printer_name: printerName, output_path: null }
    });
  }
}
```

### Option 2: Event-driven (Complex)
```
1. Backend tạo job → emit "need_output_path" event
2. Frontend bắt event → show save dialog
3. User chọn path
4. Frontend gọi update_job_output_path(job_id, path)
5. Worker resume và lưu file
```

**Cons:**
- Phức tạp hơn (cần thêm state management)
- Delay giữa create job và show dialog
- Job phải wait trong queue

## Recommendation

Dùng **Option 1 (Frontend-driven)** vì:
- ✅ Simple và straightforward
- ✅ Better UX (dialog hiện ngay)
- ✅ User control (có thể cancel)
- ✅ No backend changes needed

## Current Status

Backend đã support `output_path` parameter:
- ✅ `CreateJobRequest.output_path: Option<String>`
- ✅ `PrintJob` aggregate stores output_path
- ✅ Database has `output_path` column
- ✅ `WindowsPrinterEngine` uses output_path or auto-generates

**Next step:** Update frontend để show dialog trước khi gọi `create_print_job`.
