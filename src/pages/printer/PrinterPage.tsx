import { useCallback, useEffect, useRef, useState } from "react";
import { useOutletContext } from "react-router-dom";
import styled from "@emotion/styled";
import { Box, Button, Icon, InlineStack, Text } from "@sapo/ui-components";
import { WarningIcon } from "@sapo/ui-icons";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ActionListButton } from "src/components/ActionListButton";
import { ConfirmModal } from "src/components/ConfirmModal";
import { type JobStatusPayload, onJobStatusChanged } from "src/services/event-listener";
import { clearJobHistory, getMetrics, getPrinterConfig, type MetricsDto } from "src/services/printer-service";
import type { PrinterConfig } from "src/types/printer";
import { formatDateTime } from "src/utils/datetime";
import { showErrorToast, showToast } from "src/utils/toast";

import { AppInfoModal } from "./components/AppInfoModal";
import { Overview } from "./components/Overview";
import PrinterSettingsFormModal from "./components/PrinterSettingsFormModal";

type ModalName = "config" | "clear-cache" | "app-info";

export default function PrinterPage() {
  const { forcedUpdate = false } = useOutletContext<{ forcedUpdate?: boolean }>() ?? {};
  const [modalName, setModalName] = useState<ModalName>();

  const [printerConfig, setPrinterConfig] = useState<PrinterConfig>();
  const [metrics, setMetrics] = useState<MetricsDto | null>(null);
  const [activeJobs, setActiveJobs] = useState<Map<string, JobStatusPayload>>(new Map());
  const [isClearingCache, setIsClearingCache] = useState(false);
  const removalTimeoutsRef = useRef<Map<string, number>>(new Map());

  useEffect(() => {
    if (forcedUpdate) setModalName(undefined);
  }, [forcedUpdate]);

  const openModal = (name: ModalName) => {
    if (!forcedUpdate) setModalName(name);
  };

  const openLink = (url: string) => {
    if (forcedUpdate) return;
    openUrl(url).catch(() => showErrorToast("Không mở được đường dẫn"));
  };

  const loadPrinterConfig = useCallback(async () => {
    const config = await getPrinterConfig();
    setPrinterConfig(config);
  }, []);

  const loadMetrics = useCallback(async () => {
    const result = await getMetrics();
    setMetrics(result);
  }, []);

  const handleClearCache = useCallback(async () => {
    setIsClearingCache(true);
    try {
      await clearJobHistory();
      await loadMetrics();
      setActiveJobs(new Map());
      showToast("Đã xóa dữ liệu cache");
      setModalName(undefined);
    } catch {
      showErrorToast("Không xóa được dữ liệu cache");
    } finally {
      setIsClearingCache(false);
    }
  }, [loadMetrics]);

  useEffect(() => {
    let isMounted = true;
    const removalTimeouts = removalTimeoutsRef.current;

    loadPrinterConfig().catch(() => showErrorToast("Không tải được cấu hình máy in"));
    loadMetrics();

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
          if (payload.status === "COMPLETED" || payload.status === "FAILED" || payload.status === "CANCELLED") {
            // Metrics only change when a job reaches a terminal state
            loadMetrics();

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
      } catch {
        return () => {};
      }
    };

    const listenerPromise = setupEventListener();

    return () => {
      isMounted = false;
      // Clear pending job-removal timeouts
      removalTimeouts.forEach((id) => clearTimeout(id));
      removalTimeouts.clear();
      listenerPromise.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [loadPrinterConfig, loadMetrics]);

  // Calculate real-time stats from metrics and active jobs
  const stats = {
    total: metrics?.total_jobs || 0,
    success: metrics?.completed || 0,
    failed: metrics?.failed || 0,
    printTime: metrics?.last_print_at ? formatDateTime(metrics.last_print_at) : null,
    downloadProgress: calculateDownloadProgress(),
    printProgress: calculatePrintProgress(),
  };

  function calculateDownloadProgress(): number {
    const jobsArray = Array.from(activeJobs.values());
    if (jobsArray.length === 0) return 0;

    const downloadingJobs = jobsArray.filter((j) => j.status === "QUEUED" || j.status === "DOWNLOADING");

    if (downloadingJobs.length === 0) {
      // If no active downloads, check if we have completed downloads
      const hasCompleted = jobsArray.some(
        (j) =>
          j.status === "DOWNLOADED" ||
          j.status === "RENDERING" ||
          j.status === "SUBMITTED" ||
          j.status === "PRINTING" ||
          j.status === "COMPLETED"
      );
      return hasCompleted ? 100 : 0;
    }

    const avgProgress = downloadingJobs.reduce((sum, j) => sum + (j.progress || 0), 0) / downloadingJobs.length;
    return Math.round(avgProgress);
  }

  function calculatePrintProgress(): number {
    const jobsArray = Array.from(activeJobs.values());
    if (jobsArray.length === 0) return 0;

    const printingJobs = jobsArray.filter((j) => j.status === "SUBMITTED" || j.status === "PRINTING");

    if (printingJobs.length === 0) {
      // If no active prints, check if we have completed jobs
      const hasCompleted = jobsArray.some((j) => j.status === "COMPLETED");
      return hasCompleted ? 100 : 0;
    }

    const avgProgress = printingJobs.reduce((sum, j) => sum + (j.progress || 0), 0) / printingJobs.length;
    return Math.round(avgProgress);
  }

  const clearCacheConfirmModal = !forcedUpdate && modalName === "clear-cache" && (
    <ConfirmModal
      open
      title={
        <InlineStack gap="2" blockAlign="center">
          <Text as="span" variant="headingLg">
            <Icon source={WarningIcon} tone="warning" />
          </Text>
          <Text as="span" variant="headingLg">
            Xóa cache dữ liệu?
          </Text>
        </InlineStack>
      }
      body="Bạn có xác nhận xóa cache dữ liệu không?"
      onDismiss={() => {
        if (isClearingCache) return;
        setModalName(undefined);
      }}
      confirmAction={{
        content: "Xác nhận",
        loading: isClearingCache,
        onAction: handleClearCache,
      }}
    />
  );

  const systemConfigModal = !forcedUpdate && modalName === "config" && (
    <PrinterSettingsFormModal open onClose={() => setModalName(undefined)} onSaved={loadPrinterConfig} />
  );

  const appInfoMarkup = !forcedUpdate && modalName === "app-info" && (
    <AppInfoModal open onClose={() => setModalName(undefined)} />
  );

  return (
    <Box>
      <ButtonGroupStyled>
        <ActionListButton
          plain
          actions={[
            { content: "Chỉnh sửa cấu hình", onAction: () => openModal("config") },
            { content: "Xóa dữ liệu cache", onAction: () => openModal("clear-cache") },
          ]}
        >
          Cấu hình hệ thống
        </ActionListButton>
        <ActionListButton
          plain
          actions={[
            { content: "Hướng dẫn sử dụng", onAction: () => openLink("https://example.com/huong-dan-su-dung") },
            {
              content: "Kiểm tra và xử lý sự cố",
              onAction: () => openLink("https://example.com/kiem-tra-va-xu-ly-su-co"),
            },
          ]}
        >
          Hỗ trợ
        </ActionListButton>
        <Button plain onClick={() => openModal("app-info")}>
          Thông tin
        </Button>
      </ButtonGroupStyled>
      <Overview printerConfig={printerConfig} stats={stats} />
      {appInfoMarkup}
      {clearCacheConfirmModal}
      {systemConfigModal}
    </Box>
  );
}

const ButtonGroupStyled = styled.div`
  display: flex;
  justify-content: start;
  gap: ${(p) => p.theme.spacing("8")};
  padding: ${(p) => p.theme.spacing("4")};
  background-color: #eff7fe;
  border-bottom: 1px solid #d2d6db;

  button,
  a,
  span {
    color: #4d5761 !important;
    font-weight: 550 !important;
  }
`;
