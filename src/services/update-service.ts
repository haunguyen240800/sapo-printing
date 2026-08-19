import { invoke } from "@tauri-apps/api/core";
import { emit, listen, UnlistenFn } from "@tauri-apps/api/event";

export interface UpdateCheckResponse {
  update_available: boolean;
  version: string | null;
  release_notes: string | null;
}

type UpdateAvailableHandler = (payload: UpdateCheckResponse) => void;

export type PendingUpdateResult =
  | { status: "updated"; targetVersion: string }
  | { status: "not-updated"; targetVersion: string; currentVersion: string };

const PENDING_UPDATE_VERSION_KEY = "sapo-printer.pending-update-version";

const localUpdateAvailableHandlers = new Set<UpdateAvailableHandler>();

function notifyLocalUpdateAvailable(payload: UpdateCheckResponse) {
  localUpdateAvailableHandlers.forEach((handler) => {
    try {
      handler(payload);
    } catch {
      // One subscriber must not prevent the update gate from notifying others.
    }
  });
}

export async function onUpdateAvailable(handler: UpdateAvailableHandler): Promise<UnlistenFn> {
  localUpdateAvailableHandlers.add(handler);

  let unlistenTauri: UnlistenFn | undefined;
  try {
    unlistenTauri = await listen<UpdateCheckResponse>("update-available", (event) => {
      handler(event.payload);
    });
  } catch {
    // Local checks can still enforce the update gate if Tauri event setup fails.
  }

  return () => {
    localUpdateAvailableHandlers.delete(handler);
    unlistenTauri?.();
  };
}

/** Bản mới đã tải và sẵn sàng để mở Windows installer trong user session. */
export async function onUpdateReadyToApply(handler: (version: string) => void): Promise<UnlistenFn> {
  return listen<string>("update-ready-to-apply", (event) => {
    handler(event.payload);
  });
}

export async function checkForUpdates(): Promise<UpdateCheckResponse> {
  const result = await invoke<UpdateCheckResponse>("check_for_updates");

  if (result.update_available) {
    notifyLocalUpdateAvailable(result);
    await emit("update-available", result).catch(() => {});
  }

  return result;
}

export async function installUpdate(): Promise<string> {
  return invoke<string>("install_update");
}

export async function restartApp(targetVersion: string): Promise<void> {
  rememberPendingUpdate(targetVersion);
  try {
    await invoke("restart_app");
  } catch (error) {
    clearPendingUpdate();
    throw error;
  }
}

export function getPendingUpdateResult(currentVersion: string): PendingUpdateResult | null {
  const targetVersion = getPendingUpdateTarget();

  if (!targetVersion) return null;

  if (currentVersion === targetVersion) {
    clearPendingUpdate();
    return { status: "updated", targetVersion };
  }

  return { status: "not-updated", targetVersion, currentVersion };
}

export function getPendingUpdateTarget(): string | null {
  try {
    return window.localStorage.getItem(PENDING_UPDATE_VERSION_KEY);
  } catch {
    return null;
  }
}

function rememberPendingUpdate(targetVersion: string) {
  if (!targetVersion) return;
  try {
    window.localStorage.setItem(PENDING_UPDATE_VERSION_KEY, targetVersion);
  } catch (error) {
    throw Object.assign(new Error("Không thể lưu trạng thái cập nhật."), { cause: error });
  }
}

function clearPendingUpdate() {
  try {
    window.localStorage.removeItem(PENDING_UPDATE_VERSION_KEY);
  } catch {
    // Storage availability must not affect the updater itself.
  }
}

export async function quitApp(): Promise<void> {
  return invoke("quit_app");
}
