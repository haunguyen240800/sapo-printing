import { type ComplexAction, Modal, type ModalProps } from "@sapo/ui-components";

interface Props {
  /** Tiểu đề của modal */
  title: string;
  /** Nội dung modal */
  body: React.ReactNode;
  open: boolean;
  /**
   * Ghi đè label nút Hủy.
   * Mặc định là "Hủy", trong trường hợp confirmAction destructive sẽ là "Không phải bây giờ"
   * */
  dismissText?: string;
  /** Callback khi đóng */
  onDismiss(): void;
  /** Hành động khi ấn xác nhận */
  confirmAction?: ComplexAction;
  /**
   * Kích thước modal
   * @default "small"
   * */
  size?: ModalProps["size"];
  divider?: ModalProps["divider"];
  /** Trạng thái loading của modal */
  loading?: boolean;
}

/** Helper component for confirmation modal */
export function ConfirmModal({
  open,
  title,
  body,
  dismissText,
  onDismiss,
  confirmAction,
  size = "small",
  divider = false,
  loading,
}: Props) {
  const bodyMarkup = typeof body === "string" ? <p>{body}</p> : <div>{body}</div>;
  return (
    <Modal
      open={open}
      size={size}
      onClose={onDismiss}
      divider={divider}
      title={title}
      sectioned
      loading={loading}
      primaryAction={{
        ...confirmAction,
        disabled: confirmAction?.disabled || loading,
        content: confirmAction?.content || "Xác nhận",
      }}
      secondaryActions={[
        {
          disabled: confirmAction?.loading,
          content: dismissText || (confirmAction?.destructive ? "Không phải bây giờ" : "Hủy"),
          outline: !confirmAction?.destructive,
          onAction: onDismiss,
        },
      ]}
    >
      {bodyMarkup}
    </Modal>
  );
}
