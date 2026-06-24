import React, { useState, useEffect } from 'react';
import { PrintJobCard } from './PrintJobCard';
import { JobDto } from '../../types/print-job';

interface Props {
  jobs: JobDto[];
  onCancel: (jobId: string) => void;
}

export const PrintJobTable: React.FC<Props> = ({ jobs, onCancel }) => {
  const [currentPage, setCurrentPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);

  useEffect(() => {
    setCurrentPage(1);
  }, [jobs]);

  const totalPages = Math.max(1, Math.ceil(jobs.length / pageSize));
  const startIndex = (currentPage - 1) * pageSize;
  const paginatedJobs = jobs.slice(startIndex, startIndex + pageSize);

  if (jobs.length === 0) {
    return (
      <div className="text-center py-12 text-gray-500">
        <p className="text-lg">Không có print job nào</p>
        <p className="text-sm mt-1">Các job in sẽ hiển thị ở đây</p>
      </div>
    );
  }

  return (
    <div className="w-full">
      <div className="overflow-x-auto">
        <table className="min-w-full bg-white border border-gray-200">
          <thead className="bg-gray-50">
            <tr>
              <th className="px-4 py-3 text-left text-sm font-medium text-gray-700 border-b">
                Mã job
              </th>
              <th className="px-4 py-3 text-left text-sm font-medium text-gray-700 border-b">
                Máy in
              </th>
              <th className="px-4 py-3 text-left text-sm font-medium text-gray-700 border-b">
                Trạng thái
              </th>
              <th className="px-4 py-3 text-left text-sm font-medium text-gray-700 border-b">
                Tiến trình
              </th>
              <th className="px-4 py-3 text-left text-sm font-medium text-gray-700 border-b">
                Thời gian tạo
              </th>
              <th className="px-4 py-3 text-left text-sm font-medium text-gray-700 border-b">
                Hành động
              </th>
            </tr>
          </thead>
          <tbody>
            {paginatedJobs.map((job) => (
              <tr key={job.job_id} className="hover:bg-gray-50">
                <td className="px-4 py-3 text-sm border-b">
                  <span className="font-mono">{job.job_id.slice(0, 8)}...</span>
                </td>
                <td className="px-4 py-3 text-sm border-b">{job.printer_name}</td>
                <td className="px-4 py-3 text-sm border-b">
                  <PrintJobCard job={job} />
                </td>
                <td className="px-4 py-3 text-sm border-b">
                  <div className="flex items-center gap-2">
                    <div className="flex-1 bg-gray-200 rounded-full h-2">
                      <div
                        className={`h-2 rounded-full ${progressColor(job.status)}`}
                        style={{
                          width: `${Math.min(job.progress, 100)}%`,
                          transition: 'width 0.3s ease-in-out',
                        }}
                      />
                    </div>
                    <span className="text-sm">{job.progress}%</span>
                  </div>
                </td>
                <td className="px-4 py-3 text-sm border-b">
                  {job.created_at > 0
                    ? new Date(job.created_at * 1000).toLocaleString('vi-VN')
                    : '—'}
                </td>
                <td className="px-4 py-3 text-sm border-b">
                  <div className="flex gap-2">
                    {canCancel(job.status) && (
                      <button
                        onClick={() => onCancel(job.job_id)}
                        className="text-red-600 hover:text-red-800 font-medium"
                      >
                        Hủy
                      </button>
                    )}
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {/* Pagination */}
      <div className="flex items-center justify-between mt-4 px-4">
        <div className="text-sm text-gray-700">Tổng {jobs.length} jobs</div>
        <div className="flex items-center gap-2">
          <select
            value={pageSize}
            onChange={(e) => {
              setPageSize(Number(e.target.value));
              setCurrentPage(1);
            }}
            className="border border-gray-300 rounded px-2 py-1 text-sm"
          >
            <option value={10}>10 / trang</option>
            <option value={20}>20 / trang</option>
            <option value={50}>50 / trang</option>
          </select>
          <button
            onClick={() => setCurrentPage((p) => Math.max(1, p - 1))}
            disabled={currentPage === 1}
            className="px-3 py-1 border border-gray-300 rounded text-sm disabled:opacity-50"
          >
            Trước
          </button>
          <span className="text-sm">
            Trang {currentPage} / {totalPages}
          </span>
          <button
            onClick={() => setCurrentPage((p) => Math.min(totalPages, p + 1))}
            disabled={currentPage === totalPages}
            className="px-3 py-1 border border-gray-300 rounded text-sm disabled:opacity-50"
          >
            Sau
          </button>
        </div>
      </div>
    </div>
  );
};

function canCancel(status: string): boolean {
  return ['PENDING', 'QUEUED', 'DOWNLOADED', 'SUBMITTED_TO_QUEUE', 'PRINTING'].includes(
    status
  );
}

function progressColor(status: string): string {
  switch (status) {
    case 'PENDING':
    case 'QUEUED':
      return 'bg-gray-400';
    case 'DOWNLOADED':
    case 'SUBMITTED_TO_QUEUE':
      return 'bg-blue-500';
    case 'PRINTING':
      return 'bg-green-500';
    case 'COMPLETED':
      return 'bg-green-600';
    case 'FAILED':
      return 'bg-red-500';
    case 'CANCELLED':
      return 'bg-orange-400';
    default:
      return 'bg-gray-400';
  }
}
