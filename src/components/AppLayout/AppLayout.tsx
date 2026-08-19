import { useEffect, useState } from "react";
import { Outlet } from "react-router-dom";
import { Frame } from "@sapo/ui-components";
import { getVersion } from "@tauri-apps/api/app";
import { UnlistenFn } from "@tauri-apps/api/event";
import {
  checkForUpdates,
  getPendingUpdateResult,
  getPendingUpdateTarget,
  onUpdateAvailable,
  UpdateCheckResponse,
} from "src/services/update-service";
import { showToast, ToastProvider } from "src/utils/toast";

import { ForcedUpdateModal } from "../ForcedUpdateModal";
import { PairRequestDialog } from "../PairRequestDialog";

export function AppLayout() {
  // Mọi version mới đều BẮT BUỘC (fail-open: không có mạng thì không có event → app dùng bình thường).
  const [pendingTargetAtStartup] = useState(() => getPendingUpdateTarget());
  const [forcedUpdate, setForcedUpdate] = useState(Boolean(pendingTargetAtStartup));
  const [completedUpdateVersion, setCompletedUpdateVersion] = useState<string | null>(null);
  const [updateInfo, setUpdateInfo] = useState<UpdateCheckResponse | null>(() =>
    pendingTargetAtStartup ? { update_available: true, version: pendingTargetAtStartup, release_notes: null } : null
  );

  useEffect(() => {
    const pendingTarget = pendingTargetAtStartup;
    if (!pendingTarget) return;

    const requirePendingUpdate = () => {
      setUpdateInfo({ update_available: true, version: pendingTarget, release_notes: null });
      setForcedUpdate(true);
    };

    getVersion()
      .then((currentVersion) => {
        const result = getPendingUpdateResult(currentVersion);
        if (result?.status === "updated") {
          setUpdateInfo(null);
          setForcedUpdate(false);
          setCompletedUpdateVersion(result.targetVersion);
        } else if (result?.status === "not-updated") {
          requirePendingUpdate();
        }
      })
      .catch(() => {
        requirePendingUpdate();
      });
  }, [pendingTargetAtStartup]);

  useEffect(() => {
    if (!forcedUpdate && completedUpdateVersion) {
      showToast(`Đã cập nhật thành công lên phiên bản ${completedUpdateVersion}.`, {
        id: "app-update-result",
      });
    }
  }, [completedUpdateVersion, forcedUpdate]);

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

  return (
    <Frame>
      <ToastProvider disabled={forcedUpdate}>
        {forcedUpdate ? (
          <ForcedUpdateModal updateInfo={updateInfo} />
        ) : (
          <>
            <Outlet />
            <PairRequestDialog />
          </>
        )}
      </ToastProvider>
    </Frame>
  );
}
