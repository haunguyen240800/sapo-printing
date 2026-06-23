import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Badge, Spinner } from '@sapo/ui-components';

interface PrinterStatusDto {
  status: string;
}

interface PrinterStatusProps {
  printerName: string;
}

export const PrinterStatus: React.FC<PrinterStatusProps> = ({ printerName }) => {
  const [status, setStatus] = useState<string>('Offline');
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!printerName) {
      setLoading(false);
      return;
    }

    const loadStatus = async () => {
      try {
        setLoading(true);
        const result = await invoke<PrinterStatusDto>('get_printer_status', { name: printerName });
        setStatus(result.status);
      } catch (err) {
        console.error('Failed to get printer status:', err);
        setStatus('Offline');
      } finally {
        setLoading(false);
      }
    };

    loadStatus();
  }, [printerName]);

  if (!printerName) {
    return null;
  }

  if (loading) {
    return (
      <div style={{ display: 'inline-flex', alignItems: 'center', gap: '6px' }}>
        <Spinner size="small" />
        <span style={{ fontSize: '12px', color: '#666' }}>Đang kiểm tra...</span>
      </div>
    );
  }

  const getStatusProps = (status: string): { status: 'success' | 'warning' | 'critical' | 'plain'; label: string } => {
    switch (status) {
      case 'Online':
        return { status: 'success', label: 'Trực tuyến' };
      case 'Offline':
        return { status: 'plain', label: 'Ngoại tuyến' };
      case 'Error':
        return { status: 'critical', label: 'Lỗi' };
      default:
        return { status: 'warning', label: 'Không xác định' };
    }
  };

  const statusProps = getStatusProps(status);

  return <Badge status={statusProps.status}>{statusProps.label}</Badge>;
};
