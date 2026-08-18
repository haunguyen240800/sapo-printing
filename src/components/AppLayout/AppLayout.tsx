import { useEffect, useState } from "react";
import { Outlet } from "react-router-dom";
import { Frame } from "@sapo/ui-components";
import { UnlistenFn } from "@tauri-apps/api/event";
import { onUpdateAvailable, UpdateCheckResponse } from "src/services/update-service";
import { ToastProvider } from "src/utils/toast";

import { PairRequestDialog } from "../PairRequestDialog";
import { ForcedUpdateModal } from "../ForcedUpdateModal";

export function AppLayout() {
  // Mọi version mới đều BẮT BUỘC (fail-open: không có mạng thì không có event → app dùng bình thường).
  const [forcedUpdate, setForcedUpdate] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateCheckResponse | null>(null);

  useEffect(() => {
    let cancelled = false;
    let unlistenAvailable: UnlistenFn | undefined;

    async function setup() {
      unlistenAvailable = await onUpdateAvailable((payload) => {
        if (!cancelled) {
          setUpdateInfo(payload);
          setForcedUpdate(true);
        }
      });
    }

    setup().catch(() => {});

    return () => {
      cancelled = true;
      unlistenAvailable?.();
    };
  }, []);

  return (
    <Frame>
      <ToastProvider>
        <Outlet />
        {forcedUpdate && <ForcedUpdateModal updateInfo={updateInfo} />}
        <PairRequestDialog />
      </ToastProvider>
    </Frame>
  );
}
