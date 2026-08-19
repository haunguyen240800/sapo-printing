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
  | { status: "downloading" }
  | { status: "ready-to-restart"; targetVersion: string }
  | { status: "applying"; targetVersion: string }
  | { status: "error"; message: string; stage: "check" | "download" | "apply" };

export function useAppUpdate() {
  const [state, setState] = useState<UpdateState>({ status: "idle" });
  const [isChecking, setIsChecking] = useState(false);
  const [isInstalling, setIsInstalling] = useState(false);
  const [targetVersion, setTargetVersion] = useState("");

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
      setState({ status: "error", message: String(err), stage: "check" });
    } finally {
      setIsChecking(false);
    }
  }, []);

  const installUpdate = useCallback(async () => {
    setIsInstalling(true);
    setState({ status: "downloading" });
    try {
      const version = await installUpdateService();
      setTargetVersion(version);
      setState({ status: "ready-to-restart", targetVersion: version });
    } catch (err) {
      setState({ status: "error", message: String(err), stage: "download" });
    } finally {
      setIsInstalling(false);
    }
  }, []);

  const restartApp = useCallback(async () => {
    if (!targetVersion) return;

    setIsInstalling(true);
    setState({ status: "applying", targetVersion });
    try {
      await restartAppService(targetVersion);
    } catch (err) {
      setState({ status: "error", message: String(err), stage: "apply" });
      setIsInstalling(false);
    }
  }, [targetVersion]);

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
