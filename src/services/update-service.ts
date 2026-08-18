import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

export interface UpdateCheckResponse {
  update_available: boolean;
  version: string | null;
  release_notes: string | null;
}

export async function onUpdateAvailable(handler: (payload: UpdateCheckResponse) => void): Promise<UnlistenFn> {
  return listen<UpdateCheckResponse>("update-available", (event) => {
    handler(event.payload);
  });
}

export async function onUpdateReadyToApply(handler: () => void): Promise<UnlistenFn> {
  return listen("update-ready-to-apply", () => {
    handler();
  });
}

/**
 * Service (SYSTEM) đã nhận và đang cài bản mới im lặng — app sẽ tự thoát rồi khởi động lại.
 */
export async function onUpdateInstallingAgent(handler: () => void): Promise<UnlistenFn> {
  return listen("update-installing-agent", () => {
    handler();
  });
}

export async function checkForUpdates(): Promise<UpdateCheckResponse> {
  return invoke<UpdateCheckResponse>("check_for_updates");
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
