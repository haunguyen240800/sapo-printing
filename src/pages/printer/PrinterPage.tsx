import {Box, Button} from '@sapo/ui-components';
import styled from '@emotion/styled';
import {ActionListButton} from '../../components/ActionListButton';
import {useCallback, useEffect, useRef, useState} from 'react';
import {ConfirmModal} from '../../components/ConfirmModal';
import PrinterSettingsFormModal from './components/PrinterSettingsFormModal.tsx';
import AppInfoModal from './components/AppInfoModal';
import {Overview} from './components/Overview.tsx';
import {SupportTab} from './components/SupportTab';
import {getMetrics, getPrinterConfig, type MetricsDto,} from '../../services/printer-service';
import type {PrinterConfigDto} from '../../types';
import {type JobStatusPayload, onJobStatusChanged} from '../../services/event-listener';

export default function PrinterPage() {
    const [activeTab, setActiveTab] = useState<'overview' | 'support'>('overview');
    const [modalName, setModalName] = useState<undefined | 'config' | 'clear-cache' | 'app-info'>();

    const [printerConfig, setPrinterConfig] = useState<PrinterConfigDto>();
    const [metrics, setMetrics] = useState<MetricsDto | null>(null);
    const [activeJobs, setActiveJobs] = useState<Map<string, JobStatusPayload>>(new Map());
    const metricsIntervalRef = useRef<number | null>(null);
    const removalTimeoutsRef = useRef<Map<string, number>>(new Map());

    const loadPrinterConfig = useCallback(async () => {
        try {
            const config = await getPrinterConfig();
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
        let isMounted = true;
        const removalTimeouts = removalTimeoutsRef.current;

        loadPrinterConfig().catch(err => console.error('loadPrinterConfig error:', err));
        loadMetrics().catch(err => console.error('loadMetrics error:', err));

        // Start polling metrics every 5 seconds (reduced frequency to avoid deadlock)
        metricsIntervalRef.current = window.setInterval(() => {
            loadMetrics();
        }, 5000);

        // Subscribe to job status events
        const setupEventListener = async () => {
            try {
                return await onJobStatusChanged((payload) => {
                    // Ignore events that arrive after unmount
                    if (!isMounted) return;

                    setActiveJobs((prev) => {
                        const updated = new Map(prev);
                        updated.set(payload.job_id, payload);
                        return updated;
                    });

                    // Schedule removal of terminal jobs outside the state updater
                    if (payload.status === 'COMPLETED' || payload.status === 'FAILED' || payload.status === 'CANCELLED') {
                        const existing = removalTimeouts.get(payload.job_id);
                        if (existing) clearTimeout(existing);

                        // Keep for 5 seconds to show final status
                        const timeoutId = window.setTimeout(() => {
                            removalTimeouts.delete(payload.job_id);
                            setActiveJobs((current) => {
                                const next = new Map(current);
                                next.delete(payload.job_id);
                                return next;
                            });
                        }, 5000);
                        removalTimeouts.set(payload.job_id, timeoutId);
                    }
                });
            } catch (err) {
                console.error('setupEventListener error:', err);
                return () => {
                };
            }
        };

        const listenerPromise = setupEventListener();

        return () => {
            isMounted = false;
            if (metricsIntervalRef.current) {
                clearInterval(metricsIntervalRef.current);
            }
            // Clear pending job-removal timeouts
            removalTimeouts.forEach((id) => clearTimeout(id));
            removalTimeouts.clear();
            listenerPromise.then((unlisten) => unlisten()).catch(err => console.error('cleanup error:', err));
        };
    }, [loadPrinterConfig, loadMetrics]);

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

    const systemConfigModal = modalName === 'config' &&
        <PrinterSettingsFormModal open onClose={() => setModalName(undefined)} onSaved={loadPrinterConfig}/>;

    const appInfoMarkup = modalName === 'app-info' && <AppInfoModal open onClose={() => setModalName(undefined)}/>;

    const renderTabContent = () => {
        switch (activeTab) {
            case 'support':
                return <SupportTab/>;
            default:
                return <Overview printerConfig={printerConfig} stats={stats}/>;
        }
    };

    return (
        <Box>
            <ButtonGroupStyled>
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
    background-color: #eff7fe;
    border-bottom: 1px solid #D2D6DB;
`;