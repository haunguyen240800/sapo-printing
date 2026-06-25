import {BlockStack, Box, Button, Divider, InlineGrid, Text, TextField} from '@sapo/ui-components';
import styled from '@emotion/styled';
import {ActionListButton} from '../../components/ActionListButton';
import {useState, useEffect, useCallback} from 'react';
import {ConfirmModal} from '../../components/ConfirmModal';
import SystemConfigModal from './components/SystemConfigModal.tsx';
import AppInfoTab from './components/AppInfoTab.tsx';
import PrintConfigForm from './components/PrintConfigForm.tsx';
import {PrintProgress} from './components/PrintProgress.tsx';
import {
  listPrinters,
  createPrintJob,
  // getMetrics,
  getPrinterConfig,
  // type MetricsDto,
} from '../../services/printer-service';
import type {PrinterDto, PrinterConfigDto} from '../../types';

export default function PrinterPage() {
  const [activeTab, setActiveTab] = useState<'overview' | 'print-config' | 'support'>('overview');
  const [modalName, setModalName] = useState<undefined | 'config' | 'clear-cache' | 'app-info'>();

  const [printers, setPrinters] = useState<PrinterDto[]>([]);
  const [printerConfig, setPrinterConfig] = useState<PrinterConfigDto>();
  const [testPdfUrl, setTestPdfUrl] = useState('');
  const [isTestPrinting, setIsTestPrinting] = useState(false);
  const [testPrintStatus, setTestPrintStatus] = useState<{success?: string; error?: string} | null>(null);
  // const [isLoading, setIsLoading] = useState(false);

  const loadPrinters = useCallback(async () => {
    try {
      const result = await listPrinters();
      setPrinters(result);
    } catch (err) {
      console.error('Failed to load printers:', err);
    }
  }, []);

  const loadPrinterConfig = useCallback(async () => {
    try {
      console.log('Loading printer config...');
      const config = await getPrinterConfig();
      console.log('Loaded config:', config);
      setPrinterConfig(config);
    } catch (err) {
      console.error('Failed to load printer config:', err);
    }
  }, []);

  useEffect(() => {
    loadPrinters();
    loadPrinterConfig();
  }, [loadPrinters, loadPrinterConfig]);

  const selectedPrinter = printers.find((p) => p.is_default) || printers[0];

  // const loadMetrics = useCallback(async () => {
  //   try {
  //     const result = await getMetrics();
  //     setMetrics(result);
  //   } catch (err) {
  //     console.error('Failed to load metrics:', err);
  //     // Set default metrics on error
  //     setMetrics({
  //       collected_at: Date.now() / 1000,
  //       job_metrics: {
  //         total_jobs: 0,
  //         pending: 0,
  //         queued: 0,
  //         downloaded: 0,
  //         submitted: 0,
  //         printing: 0,
  //         completed: 0,
  //         failed: 0,
  //         cancelled: 0,
  //         success_rate: 0,
  //       },
  //       queue_metrics: {
  //         current_depth: 0,
  //         avg_wait_time_secs: 0,
  //       },
  //       printer_metrics: {
  //         printers: [],
  //       },
  //       performance_metrics: {
  //         avg_job_duration_secs: 0,
  //         p50_job_duration_secs: 0,
  //         p95_job_duration_secs: 0,
  //         p99_job_duration_secs: 0,
  //         avg_download_time_secs: 0,
  //         avg_render_time_secs: 0,
  //         avg_print_time_secs: 0,
  //       },
  //     });
  //   }
  // }, []);

  // useEffect(() => {
  //   // Load data without blocking UI
  //   setIsLoading(true);
  //   Promise.all([loadPrinters(), loadMetrics()])
  //     .catch((err) => {
  //       console.error('Failed to load initial data:', err);
  //     })
  //     .finally(() => {
  //       setIsLoading(false);
  //     });
  //   // eslint-disable-next-line react-hooks/exhaustive-deps
  // }, []);

  const handleTestPrint = async () => {
    if (!testPdfUrl || !selectedPrinter) return;
    setIsTestPrinting(true);
    setTestPrintStatus(null);
    try {
      await createPrintJob([testPdfUrl], selectedPrinter.name);
      setTestPrintStatus({success: 'Đã gửi lệnh in test thành công'});
      setTestPdfUrl('');
      // await loadMetrics();
    } catch (err) {
      setTestPrintStatus({error: err as string});
    } finally {
      setIsTestPrinting(false);
    }
  };

  const stats = {
    total: 0,
    success: 0,
    failed: 0,
    printTime: null,
    downloadProgress: 0,
    printProgress: 0,
  };

  const clearCacheConfirmModal = modalName === 'clear-cache' && (
    <ConfirmModal
      open
      title="⚠️ Xóa cache dữ liệu?"
      body="Bạn có xác nhận xóa cache dữ liệu không?"
      onDismiss={() => setModalName(undefined)}
      confirmAction={{
        content: 'Xác nhận',
        onAction: () => {
          setModalName(undefined);
        },
      }}
    />
  );

  const systemConfigModal = modalName === 'config' && <SystemConfigModal open onClose={() => setModalName(undefined)} />;

  const appInfoMarkup = modalName === 'app-info' && <AppInfoTab open onClose={() => setModalName(undefined)} />;

  const renderTabContent = () => {
    switch (activeTab) {
      case 'print-config':
        return (
          <PrintConfigForm
            onSaved={() => {
              setActiveTab('overview');
              loadPrinters();
              loadPrinterConfig();
            }}
            onCancel={() => setActiveTab('overview')}
          />
        );
      case 'support':
        return (
          <Box padding="4">
            <Text as="h2">Hỗ trợ</Text>
            <Text as="p">Nội dung hỗ trợ sẽ được thêm sau</Text>
          </Box>
        );
      default:
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
                <BlockStack gap="4">
                  <Text as="h3" fontWeight="bold">In Test từ PDF</Text>
                  <InlineGrid columns="1fr auto" gap="4" alignItems="end">
                    <TextField
                        label="URL File PDF"
                        value={testPdfUrl}
                        onChange={setTestPdfUrl}
                        placeholder="https://example.com/sample.pdf"
                    />
                    <Button
                        onClick={handleTestPrint}
                        loading={isTestPrinting}
                        disabled={!testPdfUrl || !printerConfig?.printer_name}
                    >
                      In Test
                    </Button>
                  </InlineGrid>
                  {testPrintStatus?.success && <Text as="p" color="success">{testPrintStatus.success}</Text>}
                  {testPrintStatus?.error && <Text as="p" color="critical">{testPrintStatus.error}</Text>}
                </BlockStack>
                <Divider borderWidth="05" borderColor="border-subdued"/>
                <PrintProgress
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
  };

  return (
    <Box>
      {activeTab !== 'print-config' && (
        <ButtonGroupStyled>
          <Button plain onClick={() => setActiveTab('print-config')}>
            Cấu hình in
          </Button>
          <ActionListButton
            plain
            actions={[
              {content: 'Chỉnh sửa cấu hình', onAction: () => setModalName('config')},
              {content: 'Xóa dữ liệu cache', onAction: () => setModalName('clear-cache')},
              {content: 'Đường dẫn', onAction: () => console.log('Đường dẫn')},
            ]}
          >
            Cấu hình hệ thống
          </ActionListButton>
          <Button plain onClick={() => setActiveTab('support')}>
            Hỗ trợ
          </Button>
          <Button plain onClick={() => setModalName('app-info')}>
            Thông tin
          </Button>
        </ButtonGroupStyled>
      )}
      {renderTabContent()}
      {appInfoMarkup}
      {clearCacheConfirmModal}
      {systemConfigModal}
    </Box>
  );
}

const ButtonGroupStyled = styled.div`
  display: flex;
  justify-content: start;
  gap: ${(p) => p.theme.spacing('8')};
  padding: ${(p) => p.theme.spacing('4')};
  background-color: #f2f9ff;
`;