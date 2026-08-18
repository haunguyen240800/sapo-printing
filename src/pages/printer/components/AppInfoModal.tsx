import { useEffect, useState } from "react";
import {
  Banner,
  BlockStack,
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
  const { state, checkUpdate, installUpdate, isChecking, isInstalling, isBusy } = useAppUpdate();
  const [version, setVersion] = useState("");

  useEffect(() => {
    getVersion()
      .then(setVersion)
      .catch(() => setVersion(""));
  }, []);

  const renderBanner = () => {
    switch (state.status) {
      case "idle":
        return (
          <Banner tone="info" hideDismiss>
            Phiên bản mới nhất đã được cập nhật
          </Banner>
        );

      case "checking":
        return (
          <Banner tone="info" hideDismiss>
            <InlineStack gap="2" blockAlign="center">
              <Text as="span">Đang kiểm tra phiên bản mới...</Text>
            </InlineStack>
          </Banner>
        );

      case "up-to-date":
        return (
          <Banner tone="success" hideDismiss>
            <BlockStack gap="1">
              <Text as="p" fontWeight="medium">
                Phiên bản mới nhất đã được cập nhật
              </Text>
              <Text as="p" tone="subdued">
                Bạn đang dùng phiên bản {version || state.version}
              </Text>
            </BlockStack>
          </Banner>
        );

      case "update-available":
        return (
          <Banner tone="warning" hideDismiss>
            <BlockStack gap="1">
              <Text as="p" fontWeight="medium">
                Có phiên bản mới: {state.result.version}
              </Text>
              {state.result.release_notes && (
                <Text as="p" tone="subdued">
                  {state.result.release_notes}
                </Text>
              )}
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

  const renderPrimaryAction = () => {
    if (state.status === "update-available") {
      return {
        content: "Tải và cài đặt",
        onAction: installUpdate,
        disabled: isBusy,
      };
    }

    return {
      content: isChecking ? "Đang kiểm tra..." : "Kiểm tra phiên bản",
      onAction: checkUpdate,
      disabled: isBusy,
      loading: isChecking,
    };
  };

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Thông tin"
      size="small"
      sectioned
      primaryAction={renderPrimaryAction()}
      secondaryActions={[
        {
          content: "Đóng",
          onAction: onClose,
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
