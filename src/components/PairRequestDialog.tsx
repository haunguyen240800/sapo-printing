import React, { useEffect, useState } from "react";
import { BlockStack, Icon, InlineStack, Modal, Text } from "@sapo/ui-components";
import { WarningIcon } from "@sapo/ui-icons";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";

import ConnectingIcon from "../assests/connecting.svg";

interface PairRequestPayload {
  request_id: string;
  origin: string;
}

export const PairRequestDialog: React.FC = () => {
  const [request, setRequest] = useState<PairRequestPayload | null>(null);

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;

    const setupListener = async () => {
      unlisten = await listen<PairRequestPayload>("agent-pair-request", (event) => {
        console.log("Pair request received:", event.payload);
        setRequest(event.payload);
      });
    };

    setupListener();

    return () => {
      unlisten?.();
    };
  }, []);

  const handleResolve = async (allow: boolean) => {
    if (!request) return;
    try {
      await invoke("resolve_pair", {
        requestId: request.request_id,
        approved: allow,
      });
    } catch (err) {
      console.error("Failed to resolve pair request", err);
    } finally {
      setRequest(null);
    }
  };

  return (
    <Modal
      open={!!request}
      title={
        <InlineStack gap="2" blockAlign="center">
          <Text as="span" variant="headingLg">
            <Icon source={WarningIcon} tone="warning" />
          </Text>
          <Text as="span" variant="headingLg">
            Xác nhận kết nối?
          </Text>
        </InlineStack>
      }
      size="small"
      divider={false}
      onClose={() => handleResolve(false)}
      primaryAction={{
        content: "Xác nhận",
        onAction: () => handleResolve(true),
      }}
      secondaryActions={[
        {
          content: "Hủy",
          outline: true,
          onAction: () => handleResolve(false),
        },
      ]}
    >
      <BlockStack inlineAlign="center" gap="4">
        <img src={ConnectingIcon} alt="connecting" />
        <Text as="p" alignment="center">
          Chấp nhận yêu cầu kết nối với Sapo Omni ?
        </Text>
      </BlockStack>
    </Modal>
  );
};
