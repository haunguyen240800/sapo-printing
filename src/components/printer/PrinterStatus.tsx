import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

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
        <span style={{ fontSize: '12px', color: '#666' }}>Đang kiểm tra...</span>
      </div>
    );
  }

  const getStatusColor = (status: string): string => {
    switch (status) {
      case 'Online':
        return '#4caf50'; // green
      case 'Offline':
        return '#9e9e9e'; // gray
      case 'Error':
        return '#f44336'; // red
      default:
        return '#9e9e9e';
    }
  };

  const getStatusLabel = (status: string): string => {
    switch (status) {
      case 'Online':
        return 'Trực tuyến';
      case 'Offline':
        return 'Ngoại tuyến';
      case 'Error':
        return 'Lỗi';
      default:
        return 'Không xác định';
    }
  };

  const color = getStatusColor(status);
  const label = getStatusLabel(status);

  return (
    <div style={{ display: 'inline-flex', alignItems: 'center', gap: '6px' }}>
      <span
        style={{
          display: 'inline-block',
          width: '8px',
          height: '8px',
          borderRadius: '50%',
          backgroundColor: color,
        }}
      />
      <span style={{ fontSize: '12px', color: '#666' }}>{label}</span>
    </div>
  );
};
