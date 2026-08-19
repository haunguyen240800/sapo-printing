import React, { useCallback, useEffect, useRef, useState } from "react";
import { Banner, BlockStack, Icon, InlineStack, Modal, Spinner, Text } from "@sapo/ui-components";
import { ShieldCheckIcon } from "@sapo/ui-icons";
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

  const primaryAction =
    state === "ready"
      ? { content: "Khởi động lại", onAction: handleRestart }
      : state === "error"
        ? { content: "Thử lại", onAction: runInstall }
        : undefined;

  const secondaryActions =
    state === "error" && failCount.current >= MAX_RETRIES
      ? [{ content: "Thoát ứng dụng", onAction: handleQuit }]
      : undefined;

  return (
    <Modal
      open
      title={
        <InlineStack gap="2" blockAlign="center">
          <Icon source={ShieldCheckIcon} tone="primary" />
          <Text as="span" variant="headingLg">
            Bắt buộc cập nhật
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

        {(state === "installing" || state === "agent") && (
          <InlineStack gap="2" blockAlign="center" align="center">
            <Spinner size="small" accessibilityLabel="Đang cập nhật ứng dụng" />
            <Text as="span">
              {state === "agent" ? "Đang cập nhật, ứng dụng sẽ tự khởi động lại..." : "Đang tải và cài đặt..."}
            </Text>
          </InlineStack>
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
