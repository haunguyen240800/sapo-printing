---

baseline_commit: 73fc01e
---

# Story 4.7: Create Auto-Update Popup UI

Status: done

<!-- Note: Validation is optional. Run validate-create-story for quality check before dev-story. -->

## Story

As a **user**,
I want **a clear notification when an update is available**,
So that **I can decide whether to install it now or later**.

## Acceptance Criteria

### AC-1: Create Update Event Listener Service

**Given** the frontend has `@tauri-apps/api` installed
**When** I create the update event listener service
**Then** `src/services/update-service.ts` must be created with:

```typescript
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

// Payload types matching backend UpdateCheckResponse DTO
export interface UpdateCheckResponse {
  update_available: boolean;
  version: string | null;
  release_notes: string | null;
}

// Listen for "update-available" event emitted by backend
export async function onUpdateAvailable(
  handler: (payload: UpdateCheckResponse) => void
): Promise<UnlistenFn> {
  return listen<UpdateCheckResponse>('update-available', (event) => {
    handler(event.payload);
  });
}

// Listen for "update-ready-to-apply" event emitted after download+install completes
export async function onUpdateReadyToApply(
  handler: () => void
): Promise<UnlistenFn> {
  return listen('update-ready-to-apply', () => {
    handler();
  });
}

// Invoke manual update check via Tauri command
export async function checkForUpdates(): Promise<UpdateCheckResponse> {
  return invoke<UpdateCheckResponse>('check_for_updates');
}

// Invoke update install via Tauri command
export async function installUpdate(): Promise<void> {
  return invoke('install_update');
}
```

**Pattern:** Follows existing `src/services/event-listener.ts` pattern — centralized event subscription with typed payloads and proper `UnlistenFn` cleanup returns.

**Files:**
- `src/services/update-service.ts` — **NEW** (event listeners + command invocations for update flow)

### AC-2: Create UpdatePopup Component

**Given** the update service exists
**When** I create the update popup UI
**Then** `src/components/update/UpdatePopup.tsx` must be created as a modal overlay with:

**Visual Layout:**
- Modal overlay (semi-transparent backdrop, centered card)
- App icon (use the app's existing icon or a download/update icon)
- Title: "Cập nhật phiên bản mới" (New version available)
- Current version display: "Phiên bản hiện tại: {currentVersion}"
- New version display: "Phiên bản mới: {newVersion}"
- Release notes section:
  - Header: "Có gì mới" (What's new)
  - Show first 3 bullet points from release_notes (split by newline, take first 3 non-empty lines)
  - If release_notes is null/empty, show "Không có ghi chú cho phiên bản này"
- Action buttons row:
  - **"Cập nhật ngay"** (primary Button) — triggers install_update, shows download progress
  - **"Kiểm tra phiên bản"** (secondary Button) — triggers manual check_for_updates
  - **"Đóng"** (tertiary/ghost Button) — dismisses popup

**States:**
1. **Idle** — popup visible with version info, buttons enabled
2. **Installing** — "Cập nhật ngay" button shows Spinner + "Đang tải..." text, all other buttons disabled, progress indicator visible
3. **Ready to restart** — after `"update-ready-to-apply"` event, show "Khởi động lại" (Restart) button replacing "Cập nhật ngay"
4. **Error** — if install fails, show error message with "Thử lại" (Retry) option
5. **Up to date** — after manual check finds no update, show "Bạn đang sử dụng phiên bản mới nhất" message

**Props:**
```typescript
interface Props {
  isOpen: boolean;
  onClose: () => void;
  updateInfo: UpdateCheckResponse | null;
}
```

**Styling:** Use inline `style={{}}` props (matching the `printer/` component pattern — see Dev Notes).

**Vietnamese strings:** All user-facing text hardcoded in Vietnamese (matching existing codebase pattern — no i18n framework).

**Files:**
- `src/components/update/UpdatePopup.tsx` — **NEW** (modal popup component)

### AC-3: Integrate UpdatePopup into App

**Given** UpdatePopup component exists
**When** the application starts
**Then** `src/App.tsx` must be updated to:

1. Import and render `<UpdatePopup />` alongside existing components
2. Listen for `"update-available"` events on mount (via `useEffect`)
3. Show popup when event received
4. Handle popup close (dismiss)
5. Handle manual check trigger (can be from a button in the popup or elsewhere)
6. Handle `"update-ready-to-apply"` event — update popup state to show restart button

**Integration pattern:**
```typescript
// In App.tsx or a wrapper component
const [showUpdatePopup, setShowUpdatePopup] = useState(false);
const [updateInfo, setUpdateInfo] = useState<UpdateCheckResponse | null>(null);
const [updateReady, setUpdateReady] = useState(false);

useEffect(() => {
  let unlistenAvailable: UnlistenFn | undefined;
  let unlistenReady: UnlistenFn | undefined;

  async function setup() {
    unlistenAvailable = await onUpdateAvailable((payload) => {
      setUpdateInfo(payload);
      setShowUpdatePopup(true);
    });
    unlistenReady = await onUpdateReadyToApply(() => {
      setUpdateReady(true);
    });
  }
  setup();

  return () => {
    unlistenAvailable?.();
    unlistenReady?.();
  };
}, []);
```

**CRITICAL:** Event listener cleanup must happen on unmount (return cleanup function from `useEffect`). Follow the pattern from `PrintJobDashboard.tsx`.

**Files:**
- `src/App.tsx` — **UPDATE** (add UpdatePopup rendering + event subscriptions)

### AC-4: Current Version Display

**Given** the popup is shown
**When** displaying version information
**Then** the current app version must be retrieved and displayed.

Use `@tauri-apps/api` to get the current version:
```typescript
import { getVersion } from '@tauri-apps/api/app';

const [currentVersion, setCurrentVersion] = useState<string>('');

useEffect(() => {
  getVersion().then(setCurrentVersion);
}, []);
```

Display format:
- "Phiên bản hiện tại: {currentVersion}"
- "Phiên bản mới: {updateInfo.version}"

**Files:**
- `src/components/update/UpdatePopup.tsx` — includes version display logic

### AC-5: Download Progress Indicator

**Given** user clicks "Cập nhật ngay"
**When** the update is downloading and installing
**Then** the popup must show progress feedback:

1. Call `installUpdate()` from update-service
2. Show indeterminate progress (Spinner from `@sapo/ui-components`) during download
3. Button text changes to "Đang tải..." with Spinner
4. On success (`"update-ready-to-apply"` event): show "Khởi động lại" button
5. On error: show error message, re-enable buttons

**Note:** The backend `install_update` command handles the full download+install flow. The frontend just waits for the command to resolve and for the `"update-ready-to-apply"` event. There is no chunk-level progress from the backend — use an indeterminate spinner, not a progress bar.

**Files:**
- `src/components/update/UpdatePopup.tsx` — includes install flow state management

### AC-6: Manual Update Check

**Given** the popup is open or user wants to check manually
**When** user clicks "Kiểm tra phiên bản"
**Then**:
1. Call `checkForUpdates()` from update-service
2. Show loading state during check
3. If update available → show version info
4. If no update → show "Bạn đang sử dụng phiên bản mới nhất"
5. If error → show error message

**Files:**
- `src/components/update/UpdatePopup.tsx` — includes manual check handler

### AC-7: Build Verification

**Given** all frontend changes
**When** running the dev build
**Then**:
1. `pnpm run build` must succeed (no TypeScript errors)
2. `pnpm run dev` must start without errors
3. No new npm dependencies required (all needed packages already in `package.json`)

## Tasks / Subtasks

- [x] **Task 1: Create Update Event Listener Service** (AC: #1)
  - [x] Create `src/services/update-service.ts`
  - [x] Define `UpdateCheckResponse` interface
  - [x] Implement `onUpdateAvailable()` listener
  - [x] Implement `onUpdateReadyToApply()` listener
  - [x] Implement `checkForUpdates()` command invocation
  - [x] Implement `installUpdate()` command invocation

- [x] **Task 2: Create UpdatePopup Component** (AC: #2, #4, #5, #6)
  - [x] Create `src/components/update/UpdatePopup.tsx`
  - [x] Implement modal overlay with backdrop
  - [x] Display app icon, current version, new version
  - [x] Display release notes (first 3 bullets)
  - [x] Implement "Cập nhật ngay" button with install flow
  - [x] Implement "Kiểm tra phiên bản" button with manual check
  - [x] Implement "Đóng" button to dismiss
  - [x] Implement installing state (Spinner + disabled buttons)
  - [x] Implement ready-to-restart state
  - [x] Implement error state
  - [x] Implement up-to-date state (after manual check)
  - [x] Use inline `style={{}}` for all styling

- [x] **Task 3: Integrate into App** (AC: #3)
  - [x] Update `src/App.tsx` to import UpdatePopup
  - [x] Add state for popup visibility, update info, ready-to-restart
  - [x] Add `useEffect` for event subscriptions with cleanup
  - [x] Render `<UpdatePopup />` in the component tree

- [x] **Task 4: Build Verification** (AC: #7)
  - [x] Run `pnpm run build` — no TypeScript errors
  - [x] Run `pnpm run dev` — app starts without errors
  - [x] Verify no new npm dependencies needed

## Dev Notes

### Architecture Compliance

- **No backend changes needed.** Story 4-6 already created the complete update infrastructure:
  - `infrastructure/updater/update_checker.rs` — `check_for_updates()`, `download_and_install_update()`
  - `interface/tauri/commands/update.rs` — `check_for_updates`, `install_update` Tauri commands
  - `main.rs` — updater plugin registration, startup + periodic check spawn
  - Events: `"update-available"` (with `UpdateCheckResponse` payload), `"update-ready-to-apply"` (empty payload)
- **This story is frontend-only.** All Rust/backend code is done.
- **No domain layer changes.** Auto-update is an infrastructure concern.
- **No new Tauri commands needed.** `check_for_updates` and `install_update` already exist.

### Backend Integration Points (from Story 4-6)

**Tauri Events the frontend listens to:**

| Event | Payload | When emitted |
|---|---|---|
| `"update-available"` | `{ update_available: true, version: string \| null, release_notes: string \| null }` | On startup check or periodic 24h check when update found |
| `"update-ready-to-apply"` | `()` (empty/unit) | After `download_and_install_update()` completes successfully |

**Tauri Commands the frontend invokes:**

| Command | Args | Returns | Notes |
|---|---|---|---|
| `check_for_updates` | none | `UpdateCheckResponse` | Manual check. Backend deduplicates — won't re-emit same version |
| `install_update` | none | `void` | Downloads + installs. Protected by `InstallGuard` (atomic bool) preventing concurrent installs. After success, emits `"update-ready-to-apply"` |

**Backend dedup logic:** The backend tracks `last_emitted_update_version` in a `Mutex<Option<String>>`. If the same version was already emitted, it won't emit again. After `install_update` completes, this is reset to `None` so the periodic check can re-notify if user doesn't restart.

### Frontend Patterns — Follow These

**Component structure:**
- Domain-grouped folders: `src/components/update/` (matches `src/components/print-job/`, `src/components/printer/`)
- PascalCase file names matching component names
- Named exports with `React.FC<Props>` typing
- Props interfaces defined locally at top of file

**Event listening pattern (from `src/services/event-listener.ts`):**
```typescript
import { listen, UnlistenFn } from '@tauri-apps/api/event';
export async function onEventName(handler: (payload: T) => void): Promise<UnlistenFn> {
  return listen<T>('event-name', (event) => handler(event.payload));
}
```

**Command invocation pattern (from existing components):**
```typescript
import { invoke } from '@tauri-apps/api/core';
const result = await invoke<Type>('command_name', { args });
```

**Styling — USE INLINE STYLES:**
The `printer/` components use inline `style={{}}` props and this is the working approach. Do NOT use Tailwind classes (Tailwind is not installed — classes in `print-job/` components are dead code). Do NOT use Emotion (it's in `package.json` but never imported/used).

**Vietnamese strings — hardcoded:**
All user-facing text is hardcoded in Vietnamese directly in JSX. No i18n framework. Follow this pattern.

### UI Components from @sapo/ui-components

Available components already installed (`^2.19.0`):

| Component | Use in this story |
|---|---|
| `Button` | Action buttons: "Cập nhật ngay", "Kiểm tra phiên bản", "Đóng" |
| `Spinner` | Loading indicator during install/check |
| `Banner` | Optional: error/success messages |

**No Modal/Dialog component** exists in `@sapo/ui-components`. Build the modal overlay manually:
- Fixed-position backdrop div (semi-transparent black overlay)
- Centered card div (white background, rounded corners, shadow)
- Use z-index to ensure popup is above all other content

### Current App.tsx Structure

```tsx
import PrintJobDashboard from './components/print-job/PrintJobDashboard';
import PrinterConfigForm from './components/printer/PrinterConfigForm';

function App() {
  return (
    <div>
      <PrintJobDashboard />
      <PrinterConfigForm />
    </div>
  );
}
export default App;
```

Update to add `<UpdatePopup />` and the event subscription logic. The popup should be rendered at the root level so it overlays everything.

### Release Notes Parsing

The `release_notes` field from the backend is a plain text string. To display as bullet points:
```typescript
const bullets = (release_notes || '')
  .split('\n')
  .map(line => line.replace(/^[-*•]\s*/, '').trim())
  .filter(line => line.length > 0)
  .slice(0, 3);
```

### No New npm Dependencies Required

Everything needed is already in `package.json`:
- `@tauri-apps/api` `^2.0.0` — provides `listen`, `invoke`, `getVersion`
- `@sapo/ui-components` `^2.19.0` — provides `Button`, `Spinner`
- `react` `^18` — provides `useState`, `useEffect`

**DO NOT install `@tauri-apps/plugin-updater` or `@tauri-apps/plugin-process`.** The backend handles all updater plugin interaction. The frontend communicates via Tauri events and commands.

### Pre-existing Frontend Issues (DO NOT FIX)

1. **Tailwind classes in `print-job/` components are dead code** — Tailwind is not installed. Don't try to "fix" these.
2. **`AppLayout` component is an empty stub** — not wired in. Don't touch it.
3. **`routes.tsx` is a commented-out stub** — no router installed. Don't add routing for this story.
4. **`@sapo/ui-icons` is never used** — don't introduce it just for this story unless you need an icon (use text/emoji instead if needed).

### Testing

- **No automated frontend tests exist** in this project (no test framework configured — no vitest, jest, or testing-library).
- **Manual testing only:** Verify the popup appears, buttons work, install flow completes.
- **Build verification:** `pnpm run build` must succeed with no TypeScript errors.

### Files Summary

| File | Action | Notes |
|---|---|---|
| `src/services/update-service.ts` | **NEW** | Event listeners + command invocations for update flow |
| `src/components/update/UpdatePopup.tsx` | **NEW** | Modal popup with version info, release notes, action buttons |
| `src/App.tsx` | **UPDATE** | Add UpdatePopup + event subscriptions |

### Project Structure Notes

- New files follow existing conventions: domain-grouped under `src/components/update/` and `src/services/`
- No new directories needed beyond `src/components/update/`
- No changes to `src/main.tsx` — `AppProvider` already wraps everything
- No changes to `src/index.css` — all styling via inline styles

### References

- [Source: _bmad-output/planning-artifacts/epics.md#Story 4.7] — Epic 4 story definition
- [Source: _bmad-output/planning-artifacts/prds/prd-sapo-printer-2026-06-22/prd.md#FR-4.3] — Auto-Update Popup requirements
- [Source: _bmad-output/implementation-artifacts/4-6-setup-tauri-auto-update-with-code-signing.md] — Backend infrastructure this story builds on
- [Source: src/services/event-listener.ts] — Event listener pattern to follow
- [Source: src/components/printer/PrinterConfigForm.tsx] — Inline styling pattern to follow
- [Source: src-tauri/src/interface/tauri/commands/update.rs] — Backend command implementations
- [Source: src-tauri/src/infrastructure/updater/update_checker.rs] — Backend update checker
- [Source: src-tauri/src/main.rs] — Event emission and command registration

## Dev Agent Record

### Agent Model Used

Qwen Code

### Debug Log References

No issues encountered. Build passed cleanly on first attempt.

### Completion Notes List

- **Task 1:** Created `src/services/update-service.ts` following the existing `event-listener.ts` pattern. Exports typed event listeners (`onUpdateAvailable`, `onUpdateReadyToApply`) with `UnlistenFn` cleanup returns, plus `checkForUpdates()` and `installUpdate()` command invocations.
- **Task 2:** Created `src/components/update/UpdatePopup.tsx` as a modal overlay component with 6 states (idle, installing, ready, error, up-to-date, checking). Uses inline `style={{}}` props matching the `printer/` component pattern. All user-facing text in Vietnamese. Uses `Button`, `Spinner`, `Banner` from `@sapo/ui-components`. Version display via `getVersion()` from `@tauri-apps/api/app`. Release notes parsed into first 3 bullet points.
- **Task 3:** Updated `src/App.tsx` to import and render `<UpdatePopup />` at root level. Added `useEffect` with event subscriptions (`onUpdateAvailable`, `onUpdateReadyToApply`) and proper cleanup on unmount.
- **Task 4:** `pnpm run build:web` (tsc + vite build) passed with zero errors. No new npm dependencies added.

### File List

- `src/services/update-service.ts` — NEW
- `src/components/update/UpdatePopup.tsx` — NEW
- `src/App.tsx` — UPDATED

### Change Log

- 2026-06-25: Implemented story 4-7 — auto-update popup UI with event listeners, modal component, and App integration

### Review Findings

- [x] [Review][Patch] `window.location.reload()` không restart Tauri process — Quyết định: thêm backend command `restart_app` gọi `std::process::exit(0)`. Frontend gọi `invoke('restart_app')` thay vì `window.location.reload()`.
- [x] [Review][Patch] `handleCheck` stuck ở 'checking' khi update available [UpdatePopup.tsx:62-68] — Khi `checkForUpdates()` trả về `update_available: true`, không có else branch → popupState giữ 'checking' vĩnh viễn, spinner quay vô hạn, tất cả buttons disabled.
- [x] [Review][Patch] Restart button không hiển thị khi readyToApply=true lúc mở popup [UpdatePopup.tsx:30-34] — Effect chỉ transition sang 'ready' khi `popupState === 'installing'`, nhưng `isOpen` effect reset về 'idle'. Nếu update đã download xong trước khi mở popup, user không thấy nút Restart.
- [x] [Review][Patch] Async listener registration race với cleanup [App.tsx:14-32] — `setup()` là async fire-and-forget. Nếu component unmount trước khi `await listen()` resolve, cleanup chạy khi variables vẫn undefined → listeners leak vĩnh viễn.
- [x] [Review][Patch] `getVersion()` rejection không được handle [UpdatePopup.tsx:27-29] — `getVersion().then(setCurrentVersion)` không có `.catch()`. Nếu Tauri IPC fail, unhandled promise rejection, version hiển thị rỗng.
