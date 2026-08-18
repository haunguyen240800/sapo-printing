import { useEffect, useState } from "react";
import { Outlet } from "react-router-dom";
import { Frame } from "@sapo/ui-components";
import { UnlistenFn } from "@tauri-apps/api/event";
import { checkForUpdates, onUpdateAvailable, UpdateCheckResponse } from "src/services/update-service";
import { ToastProvider } from "src/utils/toast";

import { ForcedUpdateModal } from "../ForcedUpdateModal";
import { PairRequestDialog } from "../PairRequestDialog";

export function AppLayout() {
  // Mọi version mới đều BẮT BUỘC (fail-open: không có mạng thì không có event → app dùng bình thường).
  const [forcedUpdate, setForcedUpdate] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateCheckResponse | null>(null);

  useEffect(() => {
    let cancelled = false;
    let unlistenAvailable: UnlistenFn | undefined;

    const requireUpdate = (payload: UpdateCheckResponse) => {
      if (!cancelled && payload.update_available) {
        setUpdateInfo(payload);
        setForcedUpdate(true);
      }
    };

    async function setup() {
      try {
        const unlisten = await onUpdateAvailable(requireUpdate);

        if (cancelled) {
          unlisten();
          return;
        }

        unlistenAvailable = unlisten;
      } catch {
        // The active check below still protects startup if listener setup fails.
      }

      if (cancelled) return;

      try {
        const result = await checkForUpdates();
        requireUpdate(result);
      } catch {
        // Fail open when the update server is unavailable.
      }
    }

    setup().catch(() => {});

    return () => {
      cancelled = true;
      unlistenAvailable?.();
    };
  }, []);

  if (forcedUpdate) {
    return <ForcedUpdateModal updateInfo={updateInfo} />;
  }

  return (
    <Frame>
      <ToastProvider>
        <Outlet />
        <PairRequestDialog />
      </ToastProvider>
    </Frame>
  );
}
