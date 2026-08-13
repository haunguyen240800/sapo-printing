import { useEffect, useState } from "react";
import { Outlet } from "react-router-dom";
import styled from "@emotion/styled";
import { Frame } from "@sapo/ui-components";
import { UnlistenFn } from "@tauri-apps/api/event";
import { onUpdateAvailable, onUpdateReadyToApply, UpdateCheckResponse } from "src/services/update-service";
import { ToastProvider } from "src/utils/toast";

import { PairRequestDialog } from "../PairRequestDialog";
import { UpdatePopup } from "../UpdatePopup";

export function AppLayout() {
  const [showUpdatePopup, setShowUpdatePopup] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateCheckResponse | null>(null);
  const [updateReady, setUpdateReady] = useState(false);

  useEffect(() => {
    let cancelled = false;
    let unlistenAvailable: UnlistenFn | undefined;
    let unlistenReady: UnlistenFn | undefined;

    async function setup() {
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
    }

    setup().catch(() => {});

    return () => {
      cancelled = true;
      unlistenAvailable?.();
      unlistenReady?.();
    };
  }, []);

  return (
    <FrameHost>
      <Frame>
        <ToastProvider>
          <Outlet />
          <UpdatePopup
            isOpen={showUpdatePopup}
            onClose={() => setShowUpdatePopup(false)}
            updateInfo={updateInfo}
            readyToApply={updateReady}
          />
          <PairRequestDialog />
        </ToastProvider>
      </Frame>
    </FrameHost>
  );
}

const FrameHost = styled.div`
  display: contents;
  & > div {
    display: contents;
  }
`;
