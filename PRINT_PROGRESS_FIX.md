# Print Progress Update Fix

## Problem
The `PrinterPage.tsx` component was displaying static hardcoded values for print progress instead of real-time updates when printing test files. The UI showed:
- Download progress: 0%
- Print progress: 0%
- Total jobs: 0
- Success/Failed counts: 0
- Print time: "Chưa có" (None)

## Root Cause
The component had:
1. Hardcoded static `stats` object with all zeros
2. No event listener subscription to receive job status updates from backend
3. No metrics polling to get aggregate statistics
4. Commented-out `getMetrics()` functionality

## Solution Implemented

### 1. Added Real-Time Event Listening
- Imported `onJobStatusChanged` from `event-listener.ts`
- Subscribed to `job_status_changed` Tauri events in `useEffect`
- Tracked active jobs in state using `Map<string, JobStatusPayload>`
- Auto-remove completed/failed jobs after 5 seconds

### 2. Added Metrics Polling
- Enabled `getMetrics()` import and usage
- Poll metrics every 2 seconds via `setInterval`
- Store metrics in component state
- Cleanup interval on unmount

### 3. Implemented Progress Calculations
Created two calculation functions:

**`calculateDownloadProgress()`**:
- Tracks jobs in `QUEUED` or `DOWNLOADING` status
- Averages progress across active downloads
- Returns 100% if downloads completed, 0% if no active jobs

**`calculatePrintProgress()`**:
- Tracks jobs in `SUBMITTED` or `PRINTING` status  
- Averages progress across active prints
- Returns 100% if prints completed, 0% if no active jobs

### 4. Dynamic Stats Object
Replaced hardcoded stats with real-time calculated values:
```typescript
const stats = {
  total: metrics?.job_metrics.total_jobs || 0,
  success: metrics?.job_metrics.completed || 0,
  failed: metrics?.job_metrics.failed || 0,
  printTime: metrics?.performance_metrics.avg_print_time_secs
    ? `${metrics.performance_metrics.avg_print_time_secs.toFixed(1)}s`
    : null,
  downloadProgress: calculateDownloadProgress(),
  printProgress: calculatePrintProgress(),
};
```

## Backend Event Flow (Already Implemented)

The backend emits events through the `TauriEventBus`:

1. **PrintJobCreated** → status: "PENDING", progress: 0%
2. **PrintJobQueued** → status: "QUEUED", progress: 10%
3. **PrintJobDownloaded** → status: "DOWNLOADED", progress: 40%
4. **PrintJobSubmitted** → status: "SUBMITTED_TO_QUEUE", progress: 60%
5. **PrintJobPrinting** → status: "PRINTING", progress: 80%
6. **PrintJobCompleted** → status: "COMPLETED", progress: 100%
7. **PrintJobFailed** → status: "FAILED", progress: 0%
8. **PrintJobCancelled** → status: "CANCELLED", progress: 0%

Location: `src-tauri/src/infrastructure/eventbus/tauri_event_bus.rs`

## Files Modified

### `src/pages/printer/PrinterPage.tsx`
- Added imports: `getMetrics`, `MetricsDto`, `onJobStatusChanged`, `JobStatusPayload`, `useRef`
- Added state: `metrics`, `activeJobs`, `metricsIntervalRef`
- Added function: `loadMetrics()`
- Modified `useEffect`: Added metrics polling and event listener subscription
- Modified `handleTestPrint`: Call `loadMetrics()` after job creation
- Added functions: `calculateDownloadProgress()`, `calculatePrintProgress()`
- Updated `stats` object to use real-time data

## Testing

To verify the fix works:

1. **Start the application**:
   ```bash
   npm run dev
   ```

2. **Navigate to Printer Page** (Overview tab)

3. **Enter a test PDF URL** in the "In Test từ PDF" section
   - Example: `https://www.w3.org/WAI/ER/tests/xhtml/testfiles/resources/pdf/dummy.pdf`

4. **Click "In Test" button**

5. **Observe real-time updates**:
   - Download progress bar should animate from 0% → 40%
   - Print progress bar should animate from 60% → 100%
   - Total count should increment
   - Success count should increment when completed
   - Print time should display average duration

## Expected Behavior

### During Print Job Execution:
- **Tiến trình tải xuống**: Shows 0-40% while downloading
- **Tiến trình in**: Shows 60-100% while printing
- **Tổng**: Increments with each job created
- **In thành công**: Increments when jobs complete
- **In thất bại**: Increments on errors
- **Thời gian in**: Shows average print duration in seconds

### After Completion:
- Progress bars reset to 0% after 5 seconds
- Aggregate stats persist (Total, Success, Failed, Time)
- Metrics continue updating every 2 seconds

## Architecture Notes

This implementation follows the **Event-Driven Architecture** principle:
- Backend emits domain events (`PrintJobCreated`, `PrintJobCompleted`, etc.)
- `TauriEventBus` transforms domain events to UI events (`job_status_changed`)
- Frontend subscribes via `onJobStatusChanged()` listener
- UI reactively updates based on event payloads

The separation between:
- **Real-time job tracking** (via events) for progress bars
- **Aggregate metrics** (via polling) for totals/averages

...ensures the UI remains responsive even with high job volumes.
