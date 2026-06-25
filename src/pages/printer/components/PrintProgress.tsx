import {InlineGrid, ProgressBar, Text} from '@sapo/ui-components';
import styled from "@emotion/styled";

type Props = {
    total: number;
    successful: number;
    failed: number;
    lastPrintTime: string | null;
    downloadProgress: number;
    printProgress: number;
}

export function PrintProgress({
    total,
    successful,
    failed,
    lastPrintTime,
    downloadProgress,
    printProgress
}: Props) {

    return (
        <CardWrapper>
            <InlineGrid columns="auto 1fr auto 1fr" gap="4" alignItems="center">
                {/* Hàng 1: Tiến trình tải xuống | Tiến trình in */}
                <Text as="span" variant="bodyMd">Tiến trình tải xuống:</Text>
                <ProgressBar progress={downloadProgress} size="small"/>
                <Text as="span" variant="bodyMd">Tiến trình in:</Text>
                <ProgressBar progress={printProgress} size="small"/>

                {/* Hàng 2: Thời gian in | Tổng */}
                <Text as="span" variant="bodyMd">Thời gian in:</Text>
                <Text as="span" variant="bodyMd">{lastPrintTime || 'Chưa có'}</Text>
                <Text as="span" variant="bodyMd">Tổng:</Text>
                <Text as="span" variant="bodyMd" fontWeight="bold">{total}</Text>

                {/* Hàng 3: In thành công | In thất bại */}
                <Text as="span" variant="bodyMd">In thành công:</Text>
                <Text as="span" variant="bodyMd" tone="success">{successful}</Text>
                <Text as="span" variant="bodyMd">In thất bại:</Text>
                <Text as="span" variant="bodyMd" tone="critical">{failed}</Text>
            </InlineGrid>
        </CardWrapper>
    );
}

const CardWrapper = styled.div`
    padding: 16px;
    border: 1px solid #0088FF;
    border-radius: 6px;
    background-color: #F2F9FF;
`;