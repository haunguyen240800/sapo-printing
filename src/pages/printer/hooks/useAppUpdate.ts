import { useCallback, useState } from "react";

import {
  checkForUpdates,
  installUpdate as installUpdateService,
  restartApp as restartAppService,
  type UpdateCheckResponse,
} from "../../../services/update-service";

type UpdateState =
  | { status: "idle" }
  | { status: "checking" }
  | { status: "up-to-date"; version: string }
  | { status: "update-available"; result: UpdateCheckResponse }
  | { status: "installing"; progress: number }
  | { status: "ready-to-restart" }
  | { status: "error"; message: string };

export function useAppUpdate() {
  const [state, setState] = useState<UpdateState>({ status: "idle" });
  const [isChecking, setIsChecking] = useState(false);
  const [isInstalling, setIsInstalling] = useState(false);

  const checkUpdate = useCallback(async () => {
    setIsChecking(true);
    setState({ status: "checking" });
    try {
      const result = await checkForUpdates();
      if (result.update_available && result.version) {
        setState({ status: "update-available", result });
      } else {
        setState({ status: "up-to-date", version: result.version ?? "unknown" });
      }
    } catch (err) {
      setState({ status: "error", message: err as string });
    } finally {
      setIsChecking(false);
    }
  }, []);

  const installUpdate = useCallback(async () => {
    setIsInstalling(true);
    setState({ status: "installing", progress: 0 });
    try {
      await installUpdateService();
      // Bản mới đã sẵn sàng (staged/cài xong) — chờ user bấm khởi động lại.
      setState({ status: "ready-to-restart" });
    } catch (err) {
      setState({ status: "error", message: err as string });
    } finally {
      setIsInstalling(false);
    }
  }, []);

  const restartApp = useCallback(async () => {
    try {
      await restartAppService();
    } catch {
      window.location.reload();
    }
  }, []);

  return {
    state,
    checkUpdate,
    installUpdate,
    restartApp,
    isChecking,
    isInstalling,
    isBusy: isChecking || isInstalling,
  };
}
