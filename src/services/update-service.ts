import { invoke } from "@tauri-apps/api/core";
import { emit, listen, UnlistenFn } from "@tauri-apps/api/event";

export interface UpdateCheckResponse {
  update_available: boolean;
  version: string | null;
  release_notes: string | null;
}

type UpdateAvailableHandler = (payload: UpdateCheckResponse) => void;

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

/**
 * Bản mới đã sẵn sàng (service staged im lặng, hoặc fallback UAC đã cài xong).
 * App chờ user bấm "Khởi động lại" để áp dụng.
 */
export async function onUpdateReadyToApply(handler: () => void): Promise<UnlistenFn> {
  return listen("update-ready-to-apply", () => {
    handler();
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

export async function installUpdate(): Promise<void> {
  return invoke("install_update");
}

export async function restartApp(): Promise<void> {
  return invoke("restart_app");
}

export async function quitApp(): Promise<void> {
  return invoke("quit_app");
}
