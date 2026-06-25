import { Outlet } from "react-router-dom";
import { UpdatePopup } from "../update/UpdatePopup";
import { useEffect, useState } from "react";
import { UnlistenFn } from "@tauri-apps/api/event";
import {
  UpdateCheckResponse,
  onUpdateAvailable,
  onUpdateReadyToApply,
} from "../../services/update-service";
import {ToastProvider} from "../../utils/toast/ToastProvider.tsx";
import {Frame} from "@sapo/ui-components";

export function AppLayout() {
  const [showUpdatePopup, setShowUpdatePopup] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateCheckResponse | null>(null);
  const [updateReady, setUpdateReady] = useState(false);

  useEffect(() => {
    let cancelled = false;
    let unlistenAvailable: UnlistenFn | undefined;
    let unlistenReady: UnlistenFn | undefined;

    async function setup() {
      try {
        unlistenAvailable = await onUpdateAvailable((payload) => {
          if (!cancelled) {
            setUpdateInfo(payload);
            setShowUpdatePopup(true);
          }
        });
        unlistenReady = await onUpdateReadyToApply(() => {
          if (!cancelled) {
            setUpdateReady(true);
          }
        });
      } catch (err) {
        console.error('AppLayout: setup update listeners failed:', err);
      }
    }
    setup();

    return () => {
      cancelled = true;
      unlistenAvailable?.();
      unlistenReady?.();
    };
  }, []);

  return (
    <Frame>
      <ToastProvider>
        <Outlet />
        <UpdatePopup
            isOpen={showUpdatePopup}
            onClose={() => setShowUpdatePopup(false)}
            updateInfo={updateInfo}
            readyToApply={updateReady}
        />
      </ToastProvider>

    </Frame>
  );
}
