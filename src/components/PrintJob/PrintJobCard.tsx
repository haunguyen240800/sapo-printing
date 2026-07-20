import React from 'react';
import { Badge } from '@sapo/ui-components';
import { JobDto } from '../../types/print-job';

interface Props {
  job: JobDto;
}

const STATUS_CONFIG: Record<
  string,
  { label: string; status: 'success' | 'warning' | 'critical' | 'plain' }
> = {
  PENDING: { label: 'Đang chờ', status: 'plain' },
  QUEUED: { label: 'Đang chờ', status: 'plain' },
  DOWNLOADED: { label: 'Đang tải', status: 'plain' },
  SUBMITTED_TO_QUEUE: { label: 'Đang gửi', status: 'plain' },
  PRINTING: { label: 'Đang in', status: 'warning' },
  COMPLETED: { label: 'Hoàn thành', status: 'success' },
  FAILED: { label: 'Thất bại', status: 'critical' },
  CANCELLED: { label: 'Đã hủy', status: 'warning' },
};

export const PrintJobCard: React.FC<Props> = ({ job }) => {
  const config = STATUS_CONFIG[job.status] || {
    label: job.status,
    status: 'plain' as const,
  };

  return (
    <div className="flex flex-col gap-1">
      <Badge status={config.status}>{config.label}</Badge>
      {job.error_message && (
        <span className="text-xs text-red-600">{job.error_message}</span>
      )}
    </div>
  );
};
