import React, { useCallback, useEffect, useRef, useState } from "react";
import { Button, Spinner } from "@sapo/ui-components";
import { getVersion } from "@tauri-apps/api/app";
import { UnlistenFn } from "@tauri-apps/api/event";
import {
  installUpdate,
  onUpdateInstallingAgent,
  onUpdateReadyToApply,
  quitApp,
  restartApp,
  UpdateCheckResponse,
} from "src/services/update-service";

interface Props {
  updateInfo: UpdateCheckResponse | null;
}

type State = "installing" | "agent" | "ready" | "error";

const MAX_RETRIES = 3;

/**
 * Modal bắt buộc cập nhật — chặn toàn app, KHÔNG đóng được.
 * Có version mới thì phải update xong mới dùng được.
 * - Đường chính (service SYSTEM): cài im lặng → app tự khởi động lại.
 * - Fallback (UAC): cài xong hiện nút "Khởi động lại".
 * - Lỗi: cho "Thử lại"; sau nhiều lần lỗi cho "Thoát ứng dụng".
 */
export const ForcedUpdateModal: React.FC<Props> = ({ updateInfo }) => {
  const [state, setState] = useState<State>("installing");
  const [errorMessage, setErrorMessage] = useState<string>("");
  const [currentVersion, setCurrentVersion] = useState<string>("");
  const failCount = useRef(0);

  useEffect(() => {
    getVersion()
      .then(setCurrentVersion)
      .catch(() => {});
  }, []);

  const runInstall = useCallback(async () => {
    setState("installing");
    setErrorMessage("");
    try {
      await installUpdate();
      // Nếu qua service: "update-installing-agent" sẽ tới và app tự thoát.
      // Nếu fallback UAC: "update-ready-to-apply" sẽ tới.
    } catch (err) {
      failCount.current += 1;
      setState("error");
      setErrorMessage(err instanceof Error ? err.message : String(err));
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlistenAgent: UnlistenFn | undefined;
    let unlistenReady: UnlistenFn | undefined;

    async function setup() {
      unlistenAgent = await onUpdateInstallingAgent(() => {
        if (!cancelled) setState("agent");
      });
      unlistenReady = await onUpdateReadyToApply(() => {
        if (!cancelled) setState("ready");
      });
    }

    setup().catch(() => {});
    // Tự động bắt đầu cập nhật ngay khi modal xuất hiện.
    runInstall();

    return () => {
      cancelled = true;
      unlistenAgent?.();
      unlistenReady?.();
    };
  }, [runInstall]);

  const handleRestart = async () => {
    try {
      await restartApp();
    } catch {
      window.location.reload();
    }
  };

  const handleQuit = async () => {
    try {
      await quitApp();
    } catch {
      // ignore
    }
  };

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        backgroundColor: "rgba(0, 0, 0, 0.75)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 100000,
      }}
    >
      <div
        style={{
          backgroundColor: "#fff",
          borderRadius: "8px",
          padding: "32px",
          maxWidth: "480px",
          width: "90%",
          boxShadow: "0 8px 32px rgba(0, 0, 0, 0.3)",
          textAlign: "center",
        }}
      >
        <div style={{ fontSize: "48px", marginBottom: "16px" }}>🔒</div>
        <h2 style={{ margin: "0 0 8px 0", fontSize: "20px" }}>Bắt buộc cập nhật</h2>
        <p style={{ color: "#555", marginBottom: "16px" }}>
          Đã có phiên bản mới{updateInfo?.version ? ` (${updateInfo.version})` : ""}. Vui lòng cập nhật
          để tiếp tục sử dụng.
          {currentVersion && (
            <>
              <br />
              <span style={{ fontSize: "13px", color: "#888" }}>
                Phiên bản hiện tại: {currentVersion}
              </span>
            </>
          )}
        </p>

        {(state === "installing" || state === "agent") && (
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              gap: "8px",
              marginBottom: "16px",
              color: "#1976d2",
            }}
          >
            <Spinner size="small" />
            <span>
              {state === "agent"
                ? "Đang cập nhật, ứng dụng sẽ tự khởi động lại..."
                : "Đang tải và cài đặt..."}
            </span>
          </div>
        )}

        {state === "error" && (
          <div
            style={{
              backgroundColor: "#fdecea",
              color: "#b71c1c",
              borderRadius: "4px",
              padding: "12px",
              marginBottom: "16px",
              fontSize: "14px",
              wordBreak: "break-word",
            }}
          >
            Cập nhật thất bại: {errorMessage}
          </div>
        )}

        <div style={{ display: "flex", gap: "8px", justifyContent: "center", flexWrap: "wrap" }}>
          {state === "ready" && (
            <Button primary onClick={handleRestart}>
              Khởi động lại
            </Button>
          )}
          {state === "error" && (
            <>
              <Button primary onClick={runInstall}>
                Thử lại
              </Button>
              {failCount.current >= MAX_RETRIES && (
                <Button onClick={handleQuit}>Thoát ứng dụng</Button>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
};
