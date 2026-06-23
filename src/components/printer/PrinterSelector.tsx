import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Select, Spinner } from '@sapo/ui-components';
import { PrinterDto } from '../../types/printer';

interface PrinterSelectorProps {
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
  error?: string;
}

export const PrinterSelector: React.FC<PrinterSelectorProps> = ({
  value,
  onChange,
  disabled = false,
  error,
}) => {
  const [printers, setPrinters] = useState<PrinterDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    const loadPrinters = async () => {
      try {
        setLoading(true);
        setLoadError(null);
        const result = await invoke<PrinterDto[]>('list_printers');
        setPrinters(result);
      } catch (err) {
        setLoadError(err as string);
        console.error('Failed to load printers:', err);
      } finally {
        setLoading(false);
      }
    };

    loadPrinters();
  }, []);

  if (loading) {
    return (
      <div style={{ padding: '8px 0', display: 'flex', alignItems: 'center', gap: '8px' }}>
        <Spinner size="small" />
        <span>Đang tải...</span>
      </div>
    );
  }

  if (loadError) {
    return (
      <div style={{ padding: '8px 0', color: '#d32f2f' }}>
        Không tìm thấy máy in. Vui lòng kết nối máy in.
      </div>
    );
  }

  if (printers.length === 0) {
    return (
      <div style={{ padding: '8px 0', color: '#d32f2f' }}>
        Không tìm thấy máy in. Vui lòng kết nối máy in.
      </div>
    );
  }

  return (
    <Select
      label="Chọn máy in"
      options={[
        { label: '-- Chọn máy in --', value: '' },
        ...printers.map((printer) => ({
          label: `${printer.name} (${printer.printer_type})`,
          value: printer.name,
        })),
      ]}
      value={value}
      onChange={onChange}
      disabled={disabled}
      error={error}
    />
  );
};
