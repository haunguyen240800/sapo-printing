import React from 'react';
import { Select } from '@sapo/ui-components';
import { JobFilterDto } from '../../types/print-job';

interface Props {
  filter: JobFilterDto;
  printers: string[];
  onChange: (filter: JobFilterDto) => void;
}

const STATUS_OPTIONS = [
  { label: 'Tất cả', value: '' },
  { label: 'Đang chờ', value: 'PENDING' },
  { label: 'Đang chờ xử lý', value: 'QUEUED' },
  { label: 'Đang tải', value: 'DOWNLOADED' },
  { label: 'Đang gửi', value: 'SUBMITTED_TO_QUEUE' },
  { label: 'Đang in', value: 'PRINTING' },
  { label: 'Hoàn thành', value: 'COMPLETED' },
  { label: 'Thất bại', value: 'FAILED' },
  { label: 'Đã hủy', value: 'CANCELLED' },
];

function toDateInputValue(unixTimestamp: number): string {
  const d = new Date(unixTimestamp * 1000);
  const year = d.getFullYear();
  const month = String(d.getMonth() + 1).padStart(2, '0');
  const day = String(d.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

function dateToStartOfDay(dateStr: string): number {
  const [year, month, day] = dateStr.split('-').map(Number);
  const d = new Date(year, month - 1, day, 0, 0, 0);
  return Math.floor(d.getTime() / 1000);
}

function dateToEndOfDay(dateStr: string): number {
  const [year, month, day] = dateStr.split('-').map(Number);
  const d = new Date(year, month - 1, day, 23, 59, 59);
  return Math.floor(d.getTime() / 1000);
}

export const PrintJobFilters: React.FC<Props> = ({ filter, printers, onChange }) => {
  return (
    <div className="flex gap-4 mb-4 p-4 bg-gray-50 rounded">
      <div className="flex-1">
        <label className="block text-sm font-medium mb-1">Lọc theo trạng thái</label>
        <Select
          value={filter.status || ''}
          onChange={(value) => onChange({ ...filter, status: value || undefined })}
          options={STATUS_OPTIONS}
          placeholder="Chọn trạng thái"
        />
      </div>

      <div className="flex-1">
        <label className="block text-sm font-medium mb-1">Máy in</label>
        <Select
          value={filter.printer_name || ''}
          onChange={(value) => onChange({ ...filter, printer_name: value || undefined })}
          options={[
            { label: 'Tất cả', value: '' },
            ...printers.map((p) => ({ label: p, value: p })),
          ]}
          placeholder="Chọn máy in"
        />
      </div>

      <div className="flex-1">
        <label className="block text-sm font-medium mb-1">Từ ngày</label>
        <input
          type="date"
          value={filter.from_date ? toDateInputValue(filter.from_date) : ''}
          onChange={(e) => {
            onChange({
              ...filter,
              from_date: e.target.value ? dateToStartOfDay(e.target.value) : undefined,
            });
          }}
          className="w-full border border-gray-300 rounded px-3 py-2 text-sm"
        />
      </div>

      <div className="flex-1">
        <label className="block text-sm font-medium mb-1">Đến ngày</label>
        <input
          type="date"
          value={filter.to_date ? toDateInputValue(filter.to_date) : ''}
          onChange={(e) => {
            onChange({
              ...filter,
              to_date: e.target.value ? dateToEndOfDay(e.target.value) : undefined,
            });
          }}
          className="w-full border border-gray-300 rounded px-3 py-2 text-sm"
        />
      </div>
    </div>
  );
};
