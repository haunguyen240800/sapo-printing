import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Outlet } from "react-router-dom";
import { Frame } from "@sapo/ui-components";
import { getVersion } from "@tauri-apps/api/app";
import {
  checkForUpdates,
  getPendingUpdateResult,
  getPendingUpdateTarget,
  UpdateCheckResponse,
} from "src/services/update-service";
import { showToast, ToastProvider } from "src/utils/toast";

import { ForcedUpdateModal } from "../ForcedUpdateModal";
import { PairRequestDialog } from "../PairRequestDialog";

let startupUpdateCheck: Promise<UpdateCheckResponse> | undefined;

function checkForUpdateAtStartup() {
  startupUpdateCheck ??= checkForUpdates().catch((error) => {
    startupUpdateCheck = undefined;
    throw error;
  });
  return startupUpdateCheck;
}

export function AppLayout() {
  // Chỉ kiểm tra bắt buộc khi khởi động app; kiểm tra thủ công trong AppInfoModal dùng flow riêng.
  const [pendingTargetAtStartup] = useState(() => getPendingUpdateTarget());
  const [forcedUpdate, setForcedUpdate] = useState(Boolean(pendingTargetAtStartup));
  const [completedUpdateVersion, setCompletedUpdateVersion] = useState<string | null>(null);
  const [updateInfo, setUpdateInfo] = useState<UpdateCheckResponse | null>(() =>
    pendingTargetAtStartup ? { update_available: true, version: pendingTargetAtStartup, release_notes: null } : null
  );
  const backgroundRef = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    backgroundRef.current?.toggleAttribute("inert", forcedUpdate);
  }, [forcedUpdate]);

  useLayoutEffect(() => {
    if (!forcedUpdate) return;

    const preventBackgroundEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopImmediatePropagation();
      }
    };

    document.addEventListener("keyup", preventBackgroundEscape, true);
    return () => document.removeEventListener("keyup", preventBackgroundEscape, true);
  }, [forcedUpdate]);

  useEffect(() => {
    if (!forcedUpdate && completedUpdateVersion) {
      showToast(`Đã cập nhật thành công lên phiên bản ${completedUpdateVersion}.`, {
        id: "app-update-result",
      });
    }
  }, [completedUpdateVersion, forcedUpdate]);

  useEffect(() => {
    let cancelled = false;

    const requirePendingUpdate = () => {
      if (cancelled || !pendingTargetAtStartup) return;
      setUpdateInfo({ update_available: true, version: pendingTargetAtStartup, release_notes: null });
      setForcedUpdate(true);
    };

    async function initializeUpdateGate() {
      if (pendingTargetAtStartup) {
        try {
          const currentVersion = await getVersion();
          if (cancelled) return;

          const result = getPendingUpdateResult(currentVersion, pendingTargetAtStartup);
          if (result?.status === "updated") {
            setUpdateInfo(null);
            setForcedUpdate(false);
            setCompletedUpdateVersion(result.targetVersion);
          } else if (result?.status === "not-updated") {
            requirePendingUpdate();
          }
        } catch {
          requirePendingUpdate();
        }
      }

      if (cancelled) return;

      try {
        const result = await checkForUpdateAtStartup();
        if (!cancelled && result.update_available && result.version) {
          setUpdateInfo(result);
          setForcedUpdate(true);
        }
      } catch {
        // Fail open when the update server is unavailable.
      }
    }

    initializeUpdateGate().catch(() => {});

    return () => {
      cancelled = true;
    };
  }, [pendingTargetAtStartup]);

  return (
    <Frame>
      <ToastProvider disabled={forcedUpdate}>
        <div ref={backgroundRef} aria-hidden={forcedUpdate || undefined} style={{ display: "contents" }}>
          <Outlet context={{ forcedUpdate }} />
        </div>
        {!forcedUpdate && <PairRequestDialog />}
        {forcedUpdate && <ForcedUpdateModal updateInfo={updateInfo} />}
      </ToastProvider>
    </Frame>
  );
}
