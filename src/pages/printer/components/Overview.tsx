import styled from "@emotion/styled";
import { BlockStack, Box, Divider, InlineGrid, ProgressBar, Text } from "@sapo/ui-components";

import { PaperSizeOptions } from "../../../constants/printer";
import type { PrinterConfigDto } from "../../../types";

type Props = {
  printerConfig?: PrinterConfigDto;
  stats: {
    total: number;
    success: number;
    failed: number;
    printTime: string | null;
    downloadProgress: number;
    printProgress: number;
  };
};

export function Overview({ printerConfig, stats }: Props) {
  const paperSizeLabel =
    PaperSizeOptions.find((option) => option.value === printerConfig?.paper_size)?.label ?? printerConfig?.paper_size;

  return (
    <Box padding="4">
      <BlockStack gap="4">
        <BlockStack gap="4">
          <InlineGrid columns="100px 1fr" gap="4" alignItems="center">
            <Text as="span" color="subdued">
              Máy in:
            </Text>
            <Text as="span" fontWeight="bold">
              {printerConfig?.printer_name}
            </Text>
          </InlineGrid>

          <InlineGrid columns="100px 120px 100px 80px 100px 80px" gap="4" alignItems="center">
            <Text as="span" color="subdued">
              Khổ giấy:
            </Text>
            <Text as="span" fontWeight="bold">
              {paperSizeLabel}
            </Text>
            <Text as="span" color="subdued">
              Chiều cao:
            </Text>
            <Text as="span" fontWeight="bold">
              {printerConfig?.paper_height}mm
            </Text>
            <Text as="span" color="subdued">
              Chiều rộng:
            </Text>
            <Text as="span" fontWeight="bold">
              {printerConfig?.paper_width}mm
            </Text>
          </InlineGrid>

          <InlineGrid columns="100px 120px 100px 80px 100px 80px" gap="4" alignItems="center">
            <Text as="span" color="subdued">
              Cân lề trái:
            </Text>
            <Text as="span" fontWeight="bold">
              {printerConfig?.margin_left}mm
            </Text>
            <Text as="span" color="subdued">
              Cân lề phải:
            </Text>
            <Text as="span" fontWeight="bold">
              {printerConfig?.margin_right}mm
            </Text>
          </InlineGrid>

          <InlineGrid columns="100px 120px 100px 80px 100px 80px" gap="4" alignItems="center">
            <Text as="span" color="subdued">
              Cân lề trên:
            </Text>
            <Text as="span" fontWeight="bold">
              {printerConfig?.margin_top}mm
            </Text>
            <Text as="span" color="subdued">
              Cân lề dưới:
            </Text>
            <Text as="span" fontWeight="bold">
              {printerConfig?.margin_bottom}mm
            </Text>
          </InlineGrid>
        </BlockStack>
        <Divider borderWidth="05" borderColor="border-subdued" />
        <StatsWrapper>
          <InlineGrid columns="auto 1fr auto 1fr" gap="4" alignItems="center">
            <Text as="span" variant="bodyMd">
              Tiến trình tải xuống:
            </Text>
            <ProgressBar progress={stats.downloadProgress} size="small" />
            <Text as="span" variant="bodyMd">
              Tiến trình in:
            </Text>
            <ProgressBar progress={stats.printProgress} size="small" />

            <Text as="span" variant="bodyMd">
              Thời gian in:
            </Text>
            <Text as="span" variant="bodyMd">
              {stats.printTime || "Chưa có"}
            </Text>
            <Text as="span" variant="bodyMd">
              Tổng:
            </Text>
            <Text as="span" variant="bodyMd" fontWeight="bold">
              {stats.total}
            </Text>

            <Text as="span" variant="bodyMd">
              In thành công:
            </Text>
            <Text as="span" variant="bodyMd" tone="success">
              {stats.success}
            </Text>
            <Text as="span" variant="bodyMd">
              In thất bại:
            </Text>
            <Text as="span" variant="bodyMd" tone="critical">
              {stats.failed}
            </Text>
          </InlineGrid>
        </StatsWrapper>
      </BlockStack>
    </Box>
  );
}

const StatsWrapper = styled.div`
  padding: 16px;
  border: 1px solid #0088ff;
  border-radius: 6px;
  background-color: #f2f9ff;
`;
