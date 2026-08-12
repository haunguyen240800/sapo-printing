import { BlockStack, Modal, Text } from "@sapo/ui-components";

type Props = {
  open: boolean;
  onClose: () => void;
};

export function SupportModal({ open, onClose }: Props) {
  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Hỗ trợ"
      size="small"
      sectioned
      secondaryActions={[
        {
          content: "Đóng",
          onAction: onClose,
        },
      ]}
    >
      <BlockStack gap="2">
        <Text as="p">Nội dung hỗ trợ sẽ được thêm sau</Text>
      </BlockStack>
    </Modal>
  );
}
