import { useEffect, useState } from "react";
import {
  Banner,
  BlockStack,
  Button,
  InlineGrid,
  InlineStack,
  Link,
  Modal,
  ProgressBar,
  Spinner,
  Text,
} from "@sapo/ui-components";
import { getVersion } from "@tauri-apps/api/app";

import { useAppUpdate } from "../hooks/useAppUpdate";

type Props = {
  open: boolean;
  onClose: () => void;
};

const AppInfoModal = ({ open, onClose }: Props) => {
  const { state, checkUpdate, installUpdate, isInstalling } = useAppUpdate();
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
      checkUpdate();
    }
  }, [open, checkUpdate]);

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
      case "up-to-date":
        return null;

      case "update-available":
        return (
          <Banner tone="info" hideDismiss>
            <BlockStack gap="2">
              <Text as="p">Đã có phiên bản mới ver [{state.result.version}]. Vui lòng xác nhận để cập nhật</Text>
              <InlineStack>
                <Button onClick={handleInstallUpdate}>Cập nhật</Button>
              </InlineStack>
            </BlockStack>
          </Banner>
        );

      case "installing":
        return (
          <Banner tone="info" hideDismiss>
            <BlockStack gap="2">
              <InlineStack gap="2" blockAlign="center">
                <Spinner size="small" />
                <Text as="span">
                  {state.progress < 100 ? `Đang tải xuống... ${state.progress}%` : "Đang cài đặt, vui lòng chờ..."}
                </Text>
              </InlineStack>
              <ProgressBar progress={state.progress} size="small" />
            </BlockStack>
          </Banner>
        );

      case "error":
        if (!installationStarted) {
          return null;
        }

        return (
          <Banner tone="critical" hideDismiss>
            <BlockStack gap="1">
              <Text as="p" fontWeight="medium">
                Không thể kết nối máy chủ
              </Text>
              <Text as="p" tone="subdued">
                {state.message}
              </Text>
            </BlockStack>
          </Banner>
        );
    }
  };

  return (
    <Modal
      open={open}
      onClose={handleClose}
      title="Thông tin"
      size="small"
      sectioned
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

export default AppInfoModal;
