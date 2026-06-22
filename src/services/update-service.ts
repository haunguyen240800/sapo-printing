import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

export interface UpdateCheckResponse {
  update_available: boolean;
  version: string | null;
  release_notes: string | null;
}

export type PendingUpdateResult =
  | { status: "updated"; targetVersion: string }
  | { status: "not-updated"; targetVersion: string; currentVersion: string };

const PENDING_UPDATE_VERSION_KEY = "sapo-printer.pending-update-version";

/** Bản mới đã tải và sẵn sàng để mở Windows installer trong user session. */
export async function onUpdateReadyToApply(handler: (version: string) => void): Promise<UnlistenFn> {
  return listen<string>("update-ready-to-apply", (event) => {
    handler(event.payload);
  });
}

export async function checkForUpdates(): Promise<UpdateCheckResponse> {
  return invoke<UpdateCheckResponse>("check_for_updates");
}

export async function installUpdate(): Promise<string> {
  return invoke<string>("install_update");
}

export async function restartApp(targetVersion: string): Promise<void> {
  rememberPendingUpdate(targetVersion);
  try {
    await invoke("restart_app");
  } catch (error) {
    clearPendingUpdate(targetVersion);
    throw error;
  }
}

export function getPendingUpdateResult(currentVersion: string, targetVersion: string): PendingUpdateResult | null {
  if (!targetVersion) return null;

  if (currentVersion === targetVersion) {
    clearPendingUpdate(targetVersion);
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

function clearPendingUpdate(expectedTarget?: string) {
  try {
    if (expectedTarget && window.localStorage.getItem(PENDING_UPDATE_VERSION_KEY) !== expectedTarget) return;
    window.localStorage.removeItem(PENDING_UPDATE_VERSION_KEY);
  } catch {
    // Storage availability must not affect the updater itself.
  }
}

export async function quitApp(): Promise<void> {
  return invoke("quit_app");
}
