import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

export interface UpdateCheckResponse {
  update_available: boolean;
  version: string | null;
  release_notes: string | null;
}

export async function onUpdateAvailable(
  handler: (payload: UpdateCheckResponse) => void
): Promise<UnlistenFn> {
  return listen<UpdateCheckResponse>('update-available', (event) => {
    handler(event.payload);
  });
}

export async function onUpdateReadyToApply(
  handler: () => void
): Promise<UnlistenFn> {
  return listen('update-ready-to-apply', () => {
    handler();
  });
}

export async function checkForUpdates(): Promise<UpdateCheckResponse> {
  return invoke<UpdateCheckResponse>('check_for_updates');
}

export async function installUpdate(): Promise<void> {
  return invoke('install_update');
}

export async function restartApp(): Promise<void> {
  return invoke('restart_app');
}
