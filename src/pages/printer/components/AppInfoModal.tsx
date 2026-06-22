import { useEffect, useState } from "react";
import { Banner, BlockStack, Button, InlineGrid, InlineStack, Link, Modal, Text } from "@sapo/ui-components";
import { getVersion } from "@tauri-apps/api/app";

import { useAppUpdate } from "../hooks/useAppUpdate";

type Props = {
  open: boolean;
  onClose: () => void;
};

export const AppInfoModal = ({ open, onClose }: Props) => {
  const { state, checkUpdate, installUpdate, restartApp, reset, isChecking, isInstalling } = useAppUpdate();
  const [version, setVersion] = useState("");
  const [installationStarted, setInstallationStarted] = useState(false);

  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => setVersion(""));
  }, []);

  useEffect(() => {
    if (open) {
      setInstallationStarted(false);
      reset();
    }
  }, [open, reset]);

  const handleInstallUpdate = () => {
    setInstallationStarted(true);
    installUpdate();
  };

  const handleClose = () => {
    if (!isInstalling) {
      onClose();
    }
  };

  const renderBanner = () => {
    switch (state.status) {
      case "idle":
      case "checking":
        return null;

      case "up-to-date":
        return (
          <Banner tone="info" hideDismiss>
            <Text as="p">Không có phiên bản mới</Text>
          </Banner>
        );

      case "update-available":
      case "downloading":
        return (
          <Banner tone="info" hideDismiss>
            <BlockStack gap="2">
              <Text as="p">
                {state.status === "downloading"
                  ? "Đang tải xuống bản mới..."
                  : `Đã có phiên bản mới ver [${state.result.version}]. Vui lòng xác nhận để cập nhật`}
              </Text>
              <InlineStack>
                <Button
                  variant="outline"
                  onClick={handleInstallUpdate}
                  loading={state.status === "downloading"}
                  disabled={state.status === "downloading"}
                >
                  Cập nhật
                </Button>
              </InlineStack>
            </BlockStack>
          </Banner>
        );

      case "ready-to-restart":
        return (
          <Banner tone="success" hideDismiss>
            <BlockStack gap="2">
              <Text as="p">
                Đã tải xuống bản cập nhật thành công. Nhấn “Cài đặt và khởi động lại” để bắt đầu cài đặt. Ứng dụng sẽ tự
                động khởi động lại sau khi hoàn tất.
              </Text>
              <InlineStack>
                <Button variant="primary" onClick={restartApp}>
                  Cài đặt và khởi động lại
                </Button>
              </InlineStack>
            </BlockStack>
          </Banner>
        );

      case "applying":
        return (
          <Banner tone="info" title="Đang mở trình cài đặt" hideDismiss>
            <Text as="p">Vui lòng xác nhận UAC và chờ thanh tiến trình hoàn tất. Ứng dụng sẽ tự mở lại.</Text>
          </Banner>
        );

      case "error": {
        const isCheckError = state.stage === "check";
        if (!isCheckError && !installationStarted) {
          return null;
        }

        return (
          <Banner tone="critical" hideDismiss>
            <BlockStack gap="1">
              <Text as="p" fontWeight="medium">
                {isCheckError
                  ? "Không thể kiểm tra phiên bản"
                  : state.stage === "apply"
                    ? "Không thể mở trình cài đặt"
                    : "Cập nhật thất bại"}
              </Text>
              <Text as="p" tone="subdued">
                {state.message}
              </Text>
              {!isCheckError && (
                <InlineStack>
                  <Button variant="outline" onClick={state.stage === "apply" ? restartApp : handleInstallUpdate}>
                    Thử lại
                  </Button>
                </InlineStack>
              )}
            </BlockStack>
          </Banner>
        );
      }
    }
  };

  const showCheckAction =
    state.status === "idle" ||
    state.status === "checking" ||
    state.status === "up-to-date" ||
    (state.status === "error" && state.stage === "check");

  const primaryAction = showCheckAction
    ? {
        content: "Kiểm tra phiên bản",
        onAction: checkUpdate,
        loading: isChecking,
        disabled: isChecking,
      }
    : undefined;

  return (
    <Modal
      open={open}
      onClose={handleClose}
      title="Thông tin"
      size="small"
      sectioned
      primaryAction={primaryAction}
      secondaryActions={[
        {
          content: "Đóng",
          onAction: handleClose,
          disabled: isInstalling,
        },
      ]}
    >
      <BlockStack gap="4">
        <BlockStack gap="3">
          <InlineGrid columns="120px 1fr" gap="4" alignItems="center">
            <Text as="span" tone="subdued">
              Tên ứng dụng:
            </Text>
            <Text as="span" fontWeight="bold">
              Sapo Printer Pro Max
            </Text>
          </InlineGrid>

          <InlineGrid columns="120px 1fr" gap="4" alignItems="center">
            <Text as="span" tone="subdued">
              Phiên bản:
            </Text>
            <Text as="span" fontWeight="bold">
              {version || "—"}
            </Text>
          </InlineGrid>

          <InlineGrid columns="120px 1fr" gap="4" alignItems="center">
            <Text as="span" tone="subdued">
              Website:
            </Text>
            <Link url="https://sapo.vn" target="_blank">
              sapo.vn
            </Link>
          </InlineGrid>
        </BlockStack>
        {renderBanner()}
      </BlockStack>
    </Modal>
  );
};
