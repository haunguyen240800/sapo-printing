import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
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
      <div style={{ padding: '8px 0' }}>
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
    <div>
      <label htmlFor="printer-selector" style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>
        Chọn máy in
      </label>
      <select
        id="printer-selector"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        disabled={disabled}
        style={{
          width: '100%',
          padding: '8px 12px',
          border: error ? '1px solid #d32f2f' : '1px solid #ccc',
          borderRadius: '4px',
          fontSize: '14px',
          backgroundColor: disabled ? '#f5f5f5' : '#fff',
        }}
      >
        <option value="">-- Chọn máy in --</option>
        {printers.map((printer) => (
          <option key={printer.device_id} value={printer.name}>
            {printer.name} ({printer.printer_type})
          </option>
        ))}
      </select>
      {error && (
        <div style={{ marginTop: '4px', fontSize: '12px', color: '#d32f2f' }}>
          {error}
        </div>
      )}
    </div>
  );
};
