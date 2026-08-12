import React, { useEffect, useState } from "react";
import { Banner, Button, Spinner } from "@sapo/ui-components";
import { getVersion } from "@tauri-apps/api/app";

import { checkForUpdates, installUpdate, restartApp, UpdateCheckResponse } from "../services/update-service";

interface Props {
  isOpen: boolean;
  onClose: () => void;
  updateInfo: UpdateCheckResponse | null;
  readyToApply?: boolean;
}

type PopupState = "idle" | "installing" | "ready" | "error" | "up-to-date" | "checking";

export const UpdatePopup: React.FC<Props> = ({ isOpen, onClose, updateInfo, readyToApply = false }) => {
  const [currentVersion, setCurrentVersion] = useState<string>("");
  const [popupState, setPopupState] = useState<PopupState>("idle");
  const [errorMessage, setErrorMessage] = useState<string>("");

  useEffect(() => {
    getVersion()
      .then(setCurrentVersion)
      .catch(() => {
        // Silently ignore - version will display as empty
      });
  }, []);

  useEffect(() => {
    if (readyToApply && (popupState === "installing" || popupState === "idle")) {
      setPopupState("ready");
    }
  }, [readyToApply, popupState]);

  useEffect(() => {
    if (isOpen) {
      setPopupState("idle");
      setErrorMessage("");
    }
  }, [isOpen]);

  if (!isOpen) return null;

  const releaseNotesBullets = (updateInfo?.release_notes || "")
    .split("\n")
    .map((line) => line.replace(/^[-*•]\s*/, "").trim())
    .filter((line) => line.length > 0)
    .slice(0, 3);

  const handleInstall = async () => {
    setPopupState("installing");
    setErrorMessage("");
    try {
      await installUpdate();
    } catch (err) {
      setPopupState("error");
      setErrorMessage(err instanceof Error ? err.message : String(err));
    }
  };

  const handleCheck = async () => {
    setPopupState("checking");
    setErrorMessage("");
    try {
      const result = await checkForUpdates();
      if (!result.update_available) {
        setPopupState("up-to-date");
      } else {
        setPopupState("idle");
      }
    } catch (err) {
      setPopupState("error");
      setErrorMessage(err instanceof Error ? err.message : String(err));
    }
  };

  const handleRestart = async () => {
    try {
      await restartApp();
    } catch {
      // If restart fails, fallback to webview reload
      window.location.reload();
    }
  };

  const isBusy = popupState === "installing" || popupState === "checking";

  return (
    <div
      style={{
        position: "fixed",
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        backgroundColor: "rgba(0, 0, 0, 0.5)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 9999,
      }}
    >
      <div
        style={{
          backgroundColor: "#fff",
          borderRadius: "8px",
          padding: "32px",
          maxWidth: "480px",
          width: "90%",
          boxShadow: "0 8px 32px rgba(0, 0, 0, 0.2)",
        }}
      >
        {/* Icon */}
        <div style={{ textAlign: "center", marginBottom: "16px", fontSize: "48px" }}>🔄</div>

        {/* Title */}
        <h2 style={{ textAlign: "center", margin: "0 0 8px 0", fontSize: "20px" }}>Cập nhật phiên bản mới</h2>

        {/* Version info */}
        {popupState === "up-to-date" ? (
          <p style={{ textAlign: "center", color: "#2e7d32", marginBottom: "16px" }}>
            Bạn đang sử dụng phiên bản mới nhất
          </p>
        ) : (
          <div style={{ textAlign: "center", marginBottom: "16px", color: "#555" }}>
            <p style={{ margin: "4px 0" }}>Phiên bản hiện tại: {currentVersion}</p>
            {updateInfo?.version && (
              <p style={{ margin: "4px 0", fontWeight: 600 }}>Phiên bản mới: {updateInfo.version}</p>
            )}
          </div>
        )}

        {/* Release notes */}
        {popupState !== "up-to-date" && (
          <div
            style={{
              backgroundColor: "#f5f5f5",
              borderRadius: "4px",
              padding: "12px 16px",
              marginBottom: "16px",
            }}
          >
            <h3 style={{ margin: "0 0 8px 0", fontSize: "14px", fontWeight: 600 }}>Có gì mới</h3>
            {releaseNotesBullets.length > 0 ? (
              <ul style={{ margin: 0, paddingLeft: "20px" }}>
                {releaseNotesBullets.map((bullet) => (
                  <li key={bullet} style={{ marginBottom: "4px", fontSize: "14px", color: "#333" }}>
                    {bullet}
                  </li>
                ))}
              </ul>
            ) : (
              <p style={{ margin: 0, fontSize: "14px", color: "#888" }}>Không có ghi chú cho phiên bản này</p>
            )}
          </div>
        )}

        {/* Error message */}
        {popupState === "error" && errorMessage && (
          <Banner status="critical" onDismiss={() => setErrorMessage("")}>
            {errorMessage}
          </Banner>
        )}

        {/* Installing state */}
        {popupState === "installing" && (
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
            <span>Đang tải...</span>
          </div>
        )}

        {/* Checking state */}
        {popupState === "checking" && (
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
            <span>Đang kiểm tra...</span>
          </div>
        )}

        {/* Action buttons */}
        <div style={{ display: "flex", gap: "8px", justifyContent: "center", flexWrap: "wrap" }}>
          {popupState === "ready" ? (
            <Button primary onClick={handleRestart}>
              Khởi động lại
            </Button>
          ) : (
            <Button primary onClick={handleInstall} disabled={isBusy} loading={popupState === "installing"}>
              Cập nhật ngay
            </Button>
          )}

          <Button onClick={handleCheck} disabled={isBusy}>
            Kiểm tra phiên bản
          </Button>

          <Button onClick={onClose} disabled={isBusy}>
            Đóng
          </Button>
        </div>

        {/* Retry link for error state */}
        {popupState === "error" && (
          <div style={{ textAlign: "center", marginTop: "12px" }}>
            <Button onClick={handleInstall}>Thử lại</Button>
          </div>
        )}
      </div>
    </div>
  );
};
