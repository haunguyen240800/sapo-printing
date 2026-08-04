import {BlockStack, Box, Button, Divider, InlineGrid, Text, TextField} from '@sapo/ui-components';
import styled from '@emotion/styled';
import {ActionListButton} from '../../components/ActionListButton';
import {useState, useEffect, useCallback, useRef} from 'react';
import {ConfirmModal} from '../../components/ConfirmModal';
import ConnectionConfigModal from './components/ConnectionConfigModal.tsx';
import AppInfoModal from './components/AppInfoModal.tsx';
import PrinterSettingsForm from './components/PrinterSettingsForm.tsx';
import {PrintJobStats} from './components/PrintJobStats.tsx';
import {
  listPrinters,
  createPrintJob,
  getMetrics,
  getPrinterConfig,
  type MetricsDto,
} from '../../services/printer-service';
import type {PrinterDto, PrinterConfigDto} from '../../types';
import {onJobStatusChanged, type JobStatusPayload} from '../../services/event-listener';

export default function PrinterPage() {
  const [activeTab, setActiveTab] = useState<'overview' | 'print-config' | 'support'>('overview');
  const [modalName, setModalName] = useState<undefined | 'config' | 'clear-cache' | 'app-info'>();

  const [printers, setPrinters] = useState<PrinterDto[]>([]);
  const [printerConfig, setPrinterConfig] = useState<PrinterConfigDto>();
  const [testPdfUrl, setTestPdfUrl] = useState('');
  const [isTestPrinting, setIsTestPrinting] = useState(false);
  const [testPrintStatus, setTestPrintStatus] = useState<{success?: string; error?: string} | null>(null);
  const [metrics, setMetrics] = useState<MetricsDto | null>(null);
  const [activeJobs, setActiveJobs] = useState<Map<string, JobStatusPayload>>(new Map());
  const metricsIntervalRef = useRef<number | null>(null);

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

  const loadMetrics = useCallback(async () => {
    try {
      const result = await getMetrics();
      setMetrics(result);
    } catch (err) {
      console.error('Failed to load metrics:', err);
    }
  }, []);

  useEffect(() => {
    console.log('PrinterPage: useEffect mounting');

    // Test one by one
    loadPrinters().catch(err => console.error('loadPrinters error:', err));
    loadPrinterConfig().catch(err => console.error('loadPrinterConfig error:', err));
    loadMetrics().catch(err => console.error('loadMetrics error:', err));

    // Start polling metrics every 5 seconds (reduced frequency to avoid deadlock)
    metricsIntervalRef.current = window.setInterval(() => {
      loadMetrics();
    }, 5000);

    // Subscribe to job status events
    const setupEventListener = async () => {
      try {
        const unlisten = await onJobStatusChanged((payload) => {
          setActiveJobs((prev) => {
            const updated = new Map(prev);
            if (payload.status === 'COMPLETED' || payload.status === 'FAILED' || payload.status === 'CANCELLED') {
              // Remove completed/failed jobs after they finish
              setTimeout(() => {
                setActiveJobs((current) => {
                  const next = new Map(current);
                  next.delete(payload.job_id);
                  return next;
                });
              }, 5000); // Keep for 5 seconds to show final status
            }
            updated.set(payload.job_id, payload);
            return updated;
          });
        });

        return unlisten;
      } catch (err) {
        console.error('setupEventListener error:', err);
        return () => {}; // Return no-op cleanup
      }
    };

    const listenerPromise = setupEventListener();

    return () => {
      // Cleanup on unmount
      console.log('PrinterPage: useEffect cleanup');
      if (metricsIntervalRef.current) {
        clearInterval(metricsIntervalRef.current);
      }
      listenerPromise.then((unlisten) => unlisten()).catch(err => console.error('cleanup error:', err));
    };
  }, [loadPrinters, loadPrinterConfig, loadMetrics]);

  const selectedPrinter = printers.find((p) => p.is_default) || printers[0];

  const handleTestPrint = async () => {
    if (!testPdfUrl || !printerConfig?.printer_name) return;
    setIsTestPrinting(true);
    setTestPrintStatus(null);
    try {
      await createPrintJob([testPdfUrl], printerConfig.printer_name);
      setTestPrintStatus({success: 'Đã gửi lệnh in test thành công'});
      setTestPdfUrl('');
      await loadMetrics();
    } catch (err) {
      setTestPrintStatus({error: err as string});
    } finally {
      setIsTestPrinting(false);
    }
  };

  // Calculate real-time stats from metrics and active jobs
  const stats = {
    total: metrics?.job_metrics.total_jobs || 0,
    success: metrics?.job_metrics.completed || 0,
    failed: metrics?.job_metrics.failed || 0,
    printTime: metrics?.performance_metrics.avg_print_time_secs
      ? `${metrics.performance_metrics.avg_print_time_secs.toFixed(1)}s`
      : null,
    downloadProgress: calculateDownloadProgress(),
    printProgress: calculatePrintProgress(),
  };

  function calculateDownloadProgress(): number {
    const jobsArray = Array.from(activeJobs.values());
    if (jobsArray.length === 0) return 0;

    const downloadingJobs = jobsArray.filter(
      (j) => j.status === 'QUEUED' || j.status === 'DOWNLOADING'
    );

    if (downloadingJobs.length === 0) {
      // If no active downloads, check if we have completed downloads
      const hasCompleted = jobsArray.some((j) =>
        j.status === 'DOWNLOADED' || j.status === 'RENDERING' ||
        j.status === 'SUBMITTED' || j.status === 'PRINTING' || j.status === 'COMPLETED'
      );
      return hasCompleted ? 100 : 0;
    }

    const avgProgress = downloadingJobs.reduce((sum, j) => sum + (j.progress || 0), 0) / downloadingJobs.length;
    return Math.round(avgProgress);
  }

  function calculatePrintProgress(): number {
    const jobsArray = Array.from(activeJobs.values());
    if (jobsArray.length === 0) return 0;

    const printingJobs = jobsArray.filter(
      (j) => j.status === 'SUBMITTED' || j.status === 'PRINTING'
    );

    if (printingJobs.length === 0) {
      // If no active prints, check if we have completed jobs
      const hasCompleted = jobsArray.some((j) => j.status === 'COMPLETED');
      return hasCompleted ? 100 : 0;
    }

    const avgProgress = printingJobs.reduce((sum, j) => sum + (j.progress || 0), 0) / printingJobs.length;
    return Math.round(avgProgress);
  }

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

  const systemConfigModal = modalName === 'config' && <ConnectionConfigModal open onClose={() => setModalName(undefined)} />;

  const appInfoMarkup = modalName === 'app-info' && <AppInfoModal open onClose={() => setModalName(undefined)} />;

  const renderTabContent = () => {
    switch (activeTab) {
      case 'print-config':
        return (
          <PrinterSettingsForm
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