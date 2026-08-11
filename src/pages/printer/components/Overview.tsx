import {BlockStack, Box, Divider, InlineGrid, Text} from '@sapo/ui-components';
import type {PrinterConfigDto} from '../../../types';
import {PrintJobStats} from './PrintJobStats';

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
}

export function Overview({printerConfig, stats}: Props) {
    return (
        <Box padding="4">
            <BlockStack gap="4">
                <BlockStack gap="4">
                    <InlineGrid columns="100px 1fr" gap="4" alignItems="center">
                        <Text as="span" color="subdued">Máy in:</Text>
                        <Text as="span" fontWeight="bold">{printerConfig?.printer_name}</Text>
                    </InlineGrid>

                    <InlineGrid columns="100px 80px 100px 80px 100px 80px" gap="4" alignItems="center">
                        <Text as="span" color="subdued">Khổ giấy:</Text>
                        <Text as="span" fontWeight="bold">{printerConfig?.paper_size}</Text>
                        <Text as="span" color="subdued">Chiều cao:</Text>
                        <Text as="span" fontWeight="bold">{printerConfig?.paper_height}</Text>
                        <Text as="span" color="subdued">Chiều rộng:</Text>
                        <Text as="span" fontWeight="bold">{printerConfig?.paper_width}</Text>
                    </InlineGrid>

                    <InlineGrid columns="100px 80px 100px 80px 100px 80px" gap="4" alignItems="center">
                        <Text as="span" color="subdued">Cân lề trái:</Text>
                        <Text as="span" fontWeight="bold">{printerConfig?.margin_left}</Text>
                        <Text as="span" color="subdued">Cân lề phải:</Text>
                        <Text as="span" fontWeight="bold">{printerConfig?.margin_right}</Text>
                    </InlineGrid>

                    <InlineGrid columns="100px 80px 100px 80px 100px 80px" gap="4" alignItems="center">
                        <Text as="span" color="subdued">Cân lề trên:</Text>
                        <Text as="span" fontWeight="bold">{printerConfig?.margin_top}</Text>
                        <Text as="span" color="subdued">Cân lề dưới:</Text>
                        <Text as="span" fontWeight="bold">{printerConfig?.margin_bottom}</Text>
                    </InlineGrid>
                </BlockStack>
                <Divider borderWidth="05" borderColor="border-subdued"/>
                <PrintJobStats
                    total={stats.total}
                    failed={stats.failed}
                    lastPrintTime={stats.printTime}
                    successful={stats.success}
                    downloadProgress={stats.downloadProgress}
                    printProgress={stats.printProgress}
                />
            </BlockStack>
        </Box>
    );
}
