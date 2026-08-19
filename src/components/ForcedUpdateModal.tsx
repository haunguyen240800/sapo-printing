import React, { useCallback, useEffect, useRef, useState } from "react";
import { Banner, BlockStack, Icon, InlineStack, Modal, Text } from "@sapo/ui-components";
import { WarningIcon } from "@sapo/ui-icons";
import { getVersion } from "@tauri-apps/api/app";
import { UnlistenFn } from "@tauri-apps/api/event";
import {
  installUpdate,
  onUpdateReadyToApply,
  quitApp,
  restartApp,
  UpdateCheckResponse,
} from "src/services/update-service";

interface Props {
  updateInfo: UpdateCheckResponse | null;
}

type State = "idle" | "downloading" | "ready" | "applying" | "error";
type RetryStage = "download" | "apply";

const MAX_RETRIES = 3;

/**
 * Modal bắt buộc cập nhật — chặn toàn app, KHÔNG đóng được.
 * Có version mới thì phải update xong mới dùng được.
 * - Người dùng chủ động bấm "Cập nhật ngay" để bắt đầu tải + cài.
 * - Cài/staged xong → hiện nút "Khởi động lại" để user chủ động áp dụng.
 * - Lỗi: cho "Thử lại"; sau nhiều lần lỗi cho "Thoát ứng dụng".
 */
export const ForcedUpdateModal: React.FC<Props> = ({ updateInfo }) => {
  const [state, setState] = useState<State>("idle");
  const [errorMessage, setErrorMessage] = useState<string>("");
  const [currentVersion, setCurrentVersion] = useState<string>("");
  const [downloadedVersion, setDownloadedVersion] = useState(updateInfo?.version ?? "");
  const [retryStage, setRetryStage] = useState<RetryStage>("download");
  const failCount = useRef(0);

  useEffect(() => {
    getVersion()
      .then(setCurrentVersion)
      .catch(() => {});
  }, []);

  const runInstall = useCallback(async () => {
    setState("downloading");
    setErrorMessage("");
    try {
      const version = await installUpdate();
      setDownloadedVersion(version);
      setState("ready");
    } catch (err) {
      failCount.current += 1;
      setRetryStage("download");
      setState("error");
      setErrorMessage(err instanceof Error ? err.message : String(err));
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    let unlistenReady: UnlistenFn | undefined;

    async function setup() {
      unlistenReady = await onUpdateReadyToApply((version) => {
        if (!cancelled) {
          setDownloadedVersion(version);
          setState("ready");
        }
      });
    }

    setup().catch(() => {});

    return () => {
      cancelled = true;
      unlistenReady?.();
    };
  }, []);

  const handleRestart = async () => {
    if (state === "applying" || !downloadedVersion) return;

    setState("applying");
    setErrorMessage("");
    try {
      await restartApp(downloadedVersion);
    } catch (err) {
      failCount.current += 1;
      setRetryStage("apply");
      setState("error");
      setErrorMessage(err instanceof Error ? err.message : String(err));
    }
  };

  const handleQuit = async () => {
    try {
      await quitApp();
    } catch {
      // ignore
    }
  };

  const retryAction = retryStage === "apply" ? handleRestart : runInstall;
  const primaryAction =
    state === "ready" || state === "applying"
      ? {
          content: "Cài đặt và khởi động lại",
          onAction: handleRestart,
          loading: state === "applying",
          disabled: state === "applying",
        }
      : state === "error"
        ? { content: "Thử lại", onAction: retryAction }
        : {
            content: "Tải xuống",
            onAction: runInstall,
            loading: state === "downloading",
            disabled: state === "downloading",
          };

  const secondaryActions =
    state === "error" && failCount.current >= MAX_RETRIES
      ? [{ content: "Thoát ứng dụng", onAction: handleQuit }]
      : undefined;

  return (
    <Modal
      open
      title={
        <InlineStack gap="2" blockAlign="center">
          <Icon source={WarningIcon} tone="warning" />
          <Text as="span" variant="headingLg">
            Cập nhật phiên bản mới
          </Text>
        </InlineStack>
      }
      size="small"
      sectioned
      onClose={() => {}}
      primaryAction={primaryAction}
      secondaryActions={secondaryActions}
    >
      <BlockStack gap="4">
        <BlockStack gap="2">
          <Text as="p">
            Đã có phiên bản mới{updateInfo?.version ? ` (${updateInfo.version})` : ""}. Vui lòng cập nhật để tiếp tục sử
            dụng.
          </Text>
          {currentVersion && (
            <Text as="p" variant="bodySm" tone="subdued">
              Phiên bản hiện tại: {currentVersion}
            </Text>
          )}
        </BlockStack>

        {state === "downloading" && (
          <Banner tone="info" hideDismiss>
            <InlineStack gap="2" blockAlign="center">
              <Text as="span">Đang tải xuống...</Text>
            </InlineStack>
          </Banner>
        )}

        {state === "ready" && (
          <Banner tone="success" title="Đã tải xong bản mới" hideDismiss>
            <Text as="p">
              Nhấn &#34;Cài đặt và khởi động lại&#34; để mở trình cài đặt Windows. Ứng dụng sẽ tự mở lại sau khi hoàn
              tất.
            </Text>
          </Banner>
        )}

        {state === "applying" && (
          <Banner tone="info" title="Đang mở trình cài đặt" hideDismiss>
            <Text as="p">Vui lòng xác nhận UAC và chờ thanh tiến trình hoàn tất. Ứng dụng sẽ tự mở lại.</Text>
          </Banner>
        )}

        {state === "error" && (
          <Banner tone="critical" title="Cập nhật thất bại" hideDismiss>
            <Text as="p" breakWord>
              {errorMessage}
            </Text>
          </Banner>
        )}
      </BlockStack>
    </Modal>
  );
};
