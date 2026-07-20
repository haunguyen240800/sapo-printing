import React, { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { PrintJobTable } from './PrintJobTable';
import { PrintJobFilters } from './PrintJobFilters';
import { JobDto, JobFilterDto } from '../../types/print-job';
import { onJobStatusChanged, JobStatusPayload } from '../../services/event-listener';

export const PrintJobDashboard: React.FC = () => {
  const [jobs, setJobs] = useState<JobDto[]>([]);
  const [filter, setFilter] = useState<JobFilterDto>({});
  const [printers, setPrinters] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);

  const lastUpdateRef = useRef<Map<string, number>>(new Map());
  const pendingUpdateRef = useRef<
    Map<string, { payload: JobStatusPayload; timeout: ReturnType<typeof setTimeout> }>
  >(new Map());

  const applyJobUpdate = useCallback((payload: JobStatusPayload) => {
    setJobs((prev) => {
      const idx = prev.findIndex((j) => j.job_id === payload.job_id);
      if (idx === -1) return prev;
      const updated = [...prev];
      updated[idx] = {
        ...updated[idx],
        status: payload.status,
        progress: payload.progress,
        error_message: payload.error_message,
      };
      return updated;
    });
  }, []);

  const handleJobStatusChanged = useCallback(
    (payload: JobStatusPayload) => {
      const now = Date.now();
      const lastUpdate = lastUpdateRef.current.get(payload.job_id) || 0;
      const elapsed = now - lastUpdate;

      if (elapsed >= 200) {
        lastUpdateRef.current.set(payload.job_id, now);
        applyJobUpdate(payload);
      } else {
        const existing = pendingUpdateRef.current.get(payload.job_id);
        if (existing) clearTimeout(existing.timeout);

        const timeout = setTimeout(() => {
          lastUpdateRef.current.set(payload.job_id, Date.now());
          pendingUpdateRef.current.delete(payload.job_id);
          applyJobUpdate(payload);
        }, 200 - elapsed);

        pendingUpdateRef.current.set(payload.job_id, { payload, timeout });
      }
    },
    [applyJobUpdate]
  );

  const loadJobs = useCallback(async () => {
    setLoading(true);
    try {
      const result = await invoke<JobDto[]>('list_jobs', { filter });
      setJobs(result);
    } catch (error) {
      console.error('Failed to load jobs:', error);
    } finally {
      setLoading(false);
    }
  }, [filter]);

  useEffect(() => {
    loadJobs();
  }, [loadJobs]);

  useEffect(() => {
    const loadPrinters = async () => {
      try {
        const result = await invoke<Array<{ name: string }>>('list_printers');
        setPrinters(result.map((p) => p.name));
      } catch (error) {
        console.error('Failed to load printers:', error);
      }
    };
    loadPrinters();
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    const subscribe = async () => {
      try {
        const fn = await onJobStatusChanged(handleJobStatusChanged);
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      } catch (err) {
        console.error('Failed to subscribe to job events:', err);
      }
    };

    subscribe();

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
      pendingUpdateRef.current.forEach(({ timeout }) => clearTimeout(timeout));
      pendingUpdateRef.current.clear();
    };
  }, [handleJobStatusChanged]);

  const handleCancel = async (jobId: string) => {
    if (!confirm('Bạn có chắc muốn hủy job này?')) return;

    try {
      await invoke('cancel_print_job', { payload: { job_id: jobId } });
      await loadJobs();
    } catch (error) {
      alert(`Hủy job thất bại: ${error}`);
    }
  };

  return (
    <div className="p-6">
      <h1 className="text-2xl font-bold mb-4">Quản lý Print Jobs</h1>

      <PrintJobFilters filter={filter} printers={printers} onChange={setFilter} />

      {loading ? (
        <div className="text-center py-8">Đang tải...</div>
      ) : (
        <PrintJobTable jobs={jobs} onCancel={handleCancel} />
      )}
    </div>
  );
};
