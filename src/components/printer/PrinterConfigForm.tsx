import React, { useEffect, useState } from 'react';
import { useForm } from 'react-hook-form';
import { yupResolver } from '@hookform/resolvers/yup';
import * as yup from 'yup';
import { invoke } from '@tauri-apps/api/core';
import { PrinterSelector } from './PrinterSelector';
import { PrinterStatus } from './PrinterStatus';
import { PrinterDto, PrinterConfigDto } from '../../types/printer';

interface PrinterConfigFormData {
  printer_name: string;
  paper_size: string;
  paper_width: number | null | undefined;
  paper_height: number | null | undefined;
  orientation: string;
  margin_left: number;
  margin_right: number;
  margin_top: number;
  margin_bottom: number;
}

// Validation schema
const schema = yup.object({
  printer_name: yup.string().required('Tên máy in không được để trống'),
  paper_size: yup.string().required('Khổ giấy không được để trống'),
  paper_width: yup
    .number()
    .nullable()
    .transform((value, original) => (original === '' ? null : value))
    .typeError('Phải là số')
    .when('paper_size', {
      is: 'Custom',
      then: (schema) =>
        schema
          .required('Chiều rộng bắt buộc khi chọn khổ Custom')
          .min(50, 'Chiều rộng phải trong khoảng 50-500mm')
          .max(500, 'Chiều rộng phải trong khoảng 50-500mm'),
    }),
  paper_height: yup
    .number()
    .nullable()
    .transform((value, original) => (original === '' ? null : value))
    .typeError('Phải là số')
    .when('paper_size', {
      is: 'Custom',
      then: (schema) =>
        schema
          .required('Chiều cao bắt buộc khi chọn khổ Custom')
          .min(50, 'Chiều cao phải trong khoảng 50-500mm')
          .max(500, 'Chiều cao phải trong khoảng 50-500mm'),
    }),
  orientation: yup.string().required(),
  margin_left: yup
    .number()
    .typeError('Phải là số')
    .required('Lề trái không được để trống')
    .min(0, 'Lề trái phải trong khoảng 0-100mm')
    .max(100, 'Lề trái phải trong khoảng 0-100mm'),
  margin_right: yup
    .number()
    .typeError('Phải là số')
    .required('Lề phải không được để trống')
    .min(0, 'Lề phải phải trong khoảng 0-100mm')
    .max(100, 'Lề phải phải trong khoảng 0-100mm'),
  margin_top: yup
    .number()
    .typeError('Phải là số')
    .required('Lề trên không được để trống')
    .min(0, 'Lề trên phải trong khoảng 0-100mm')
    .max(100, 'Lề trên phải trong khoảng 0-100mm'),
  margin_bottom: yup
    .number()
    .typeError('Phải là số')
    .required('Lề dưới không được để trống')
    .min(0, 'Lề dưới phải trong khoảng 0-100mm')
    .max(100, 'Lề dưới phải trong khoảng 0-100mm'),
}).required();

export const PrinterConfigForm: React.FC = () => {
  const [notification, setNotification] = useState<{ type: 'success' | 'error'; message: string } | null>(null);

  const {
    register,
    handleSubmit,
    watch,
    setValue,
    formState: { errors, isSubmitting, isValid },
  } = useForm<PrinterConfigFormData>({
    resolver: yupResolver(schema) as any,
    mode: 'onChange',
    defaultValues: {
      printer_name: '',
      paper_size: 'A4',
      paper_width: undefined,
      paper_height: undefined,
      orientation: 'Portrait',
      margin_left: 0,
      margin_right: 0,
      margin_top: 0,
      margin_bottom: 0,
    },
  });

  const selectedPrinter = watch('printer_name');
  const selectedPaperSize = watch('paper_size');

  // Auto-select default printer on mount
  useEffect(() => {
    const loadDefaultPrinter = async () => {
      try {
        const printers = await invoke<PrinterDto[]>('list_printers');
        if (printers.length === 0) {
          setNotification({
            type: 'error',
            message: 'Không tìm thấy máy in. Vui lòng kết nối máy in.',
          });
          return;
        }

        // Find default printer or use first one
        const defaultPrinter = printers.find((p) => p.is_default) || printers[0];
        setValue('printer_name', defaultPrinter.name);
      } catch (err) {
        console.error('Failed to load printers:', err);
      }
    };

    loadDefaultPrinter();
  }, [setValue]);

  const onSubmit = async (data: PrinterConfigFormData) => {
    try {
      setNotification(null);
      await invoke('save_printer_config', { config: data });
      setNotification({
        type: 'success',
        message: 'Đã lưu cấu hình máy in',
      });
    } catch (err) {
      setNotification({
        type: 'error',
        message: err as string,
      });
    }
  };

  return (
    <div style={{ maxWidth: '600px', margin: '0 auto', padding: '24px' }}>
      <h1 style={{ marginBottom: '24px' }}>Cấu hình máy in</h1>

      {notification && (
        <div
          style={{
            padding: '12px 16px',
            marginBottom: '16px',
            borderRadius: '4px',
            backgroundColor: notification.type === 'success' ? '#e8f5e9' : '#ffebee',
            color: notification.type === 'success' ? '#2e7d32' : '#c62828',
          }}
        >
          {notification.message}
        </div>
      )}

      <form onSubmit={handleSubmit(onSubmit)}>
        {/* Section 1: Printer Selection */}
        <div style={{ marginBottom: '24px', padding: '16px', border: '1px solid #e0e0e0', borderRadius: '4px' }}>
          <h2 style={{ marginBottom: '16px', fontSize: '18px' }}>1. Chọn máy in</h2>
          <PrinterSelector
            value={selectedPrinter}
            onChange={(value) => setValue('printer_name', value, { shouldValidate: true })}
            error={errors.printer_name?.message}
          />
          {selectedPrinter && (
            <div style={{ marginTop: '8px' }}>
              <PrinterStatus printerName={selectedPrinter} />
            </div>
          )}
        </div>

        {/* Section 2: Paper Settings */}
        <div style={{ marginBottom: '24px', padding: '16px', border: '1px solid #e0e0e0', borderRadius: '4px' }}>
          <h2 style={{ marginBottom: '16px', fontSize: '18px' }}>2. Cài đặt giấy</h2>

          <div style={{ marginBottom: '16px' }}>
            <label htmlFor="paper_size" style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>
              Khổ giấy
            </label>
            <select
              id="paper_size"
              {...register('paper_size')}
              style={{
                width: '100%',
                padding: '8px 12px',
                border: errors.paper_size ? '1px solid #d32f2f' : '1px solid #ccc',
                borderRadius: '4px',
                fontSize: '14px',
              }}
            >
              <option value="A4">A4 (210 × 297 mm)</option>
              <option value="A5">A5 (148 × 210 mm)</option>
              <option value="Letter">Letter (216 × 279 mm)</option>
              <option value="Custom">Tùy chỉnh</option>
            </select>
            {errors.paper_size && (
              <div style={{ marginTop: '4px', fontSize: '12px', color: '#d32f2f' }}>
                {errors.paper_size.message}
              </div>
            )}
          </div>

          {selectedPaperSize === 'Custom' && (
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px', marginBottom: '16px' }}>
              <div>
                <label htmlFor="paper_width" style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>
                  Chiều rộng (mm)
                </label>
                <input
                  id="paper_width"
                  type="number"
                  {...register('paper_width')}
                  style={{
                    width: '100%',
                    padding: '8px 12px',
                    border: errors.paper_width ? '1px solid #d32f2f' : '1px solid #ccc',
                    borderRadius: '4px',
                    fontSize: '14px',
                  }}
                />
                {errors.paper_width && (
                  <div style={{ marginTop: '4px', fontSize: '12px', color: '#d32f2f' }}>
                    {errors.paper_width.message}
                  </div>
                )}
              </div>

              <div>
                <label htmlFor="paper_height" style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>
                  Chiều cao (mm)
                </label>
                <input
                  id="paper_height"
                  type="number"
                  {...register('paper_height')}
                  style={{
                    width: '100%',
                    padding: '8px 12px',
                    border: errors.paper_height ? '1px solid #d32f2f' : '1px solid #ccc',
                    borderRadius: '4px',
                    fontSize: '14px',
                  }}
                />
                {errors.paper_height && (
                  <div style={{ marginTop: '4px', fontSize: '12px', color: '#d32f2f' }}>
                    {errors.paper_height.message}
                  </div>
                )}
              </div>
            </div>
          )}

          <div>
            <label style={{ display: 'flex', alignItems: 'center', gap: '8px', cursor: 'pointer' }}>
              <input
                type="checkbox"
                checked={watch('orientation') === 'Landscape'}
                onChange={(e) => setValue('orientation', e.target.checked ? 'Landscape' : 'Portrait')}
                style={{ width: '16px', height: '16px' }}
              />
              <span style={{ fontSize: '14px' }}>In chiều ngang</span>
            </label>
          </div>
        </div>

        {/* Section 3: Layout - Margins */}
        <div style={{ marginBottom: '24px', padding: '16px', border: '1px solid #e0e0e0', borderRadius: '4px' }}>
          <h2 style={{ marginBottom: '16px', fontSize: '18px' }}>3. Lề trang</h2>

          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' }}>
            <div>
              <label htmlFor="margin_left" style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>
                Lề trái (mm)
              </label>
              <input
                id="margin_left"
                type="number"
                {...register('margin_left')}
                style={{
                  width: '100%',
                  padding: '8px 12px',
                  border: errors.margin_left ? '1px solid #d32f2f' : '1px solid #ccc',
                  borderRadius: '4px',
                  fontSize: '14px',
                }}
              />
              {errors.margin_left && (
                <div style={{ marginTop: '4px', fontSize: '12px', color: '#d32f2f' }}>
                  {errors.margin_left.message}
                </div>
              )}
            </div>

            <div>
              <label htmlFor="margin_right" style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>
                Lề phải (mm)
              </label>
              <input
                id="margin_right"
                type="number"
                {...register('margin_right')}
                style={{
                  width: '100%',
                  padding: '8px 12px',
                  border: errors.margin_right ? '1px solid #d32f2f' : '1px solid #ccc',
                  borderRadius: '4px',
                  fontSize: '14px',
                }}
              />
              {errors.margin_right && (
                <div style={{ marginTop: '4px', fontSize: '12px', color: '#d32f2f' }}>
                  {errors.margin_right.message}
                </div>
              )}
            </div>

            <div>
              <label htmlFor="margin_top" style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>
                Lề trên (mm)
              </label>
              <input
                id="margin_top"
                type="number"
                {...register('margin_top')}
                style={{
                  width: '100%',
                  padding: '8px 12px',
                  border: errors.margin_top ? '1px solid #d32f2f' : '1px solid #ccc',
                  borderRadius: '4px',
                  fontSize: '14px',
                }}
              />
              {errors.margin_top && (
                <div style={{ marginTop: '4px', fontSize: '12px', color: '#d32f2f' }}>
                  {errors.margin_top.message}
                </div>
              )}
            </div>

            <div>
              <label htmlFor="margin_bottom" style={{ display: 'block', marginBottom: '4px', fontWeight: 500 }}>
                Lề dưới (mm)
              </label>
              <input
                id="margin_bottom"
                type="number"
                {...register('margin_bottom')}
                style={{
                  width: '100%',
                  padding: '8px 12px',
                  border: errors.margin_bottom ? '1px solid #d32f2f' : '1px solid #ccc',
                  borderRadius: '4px',
                  fontSize: '14px',
                }}
              />
              {errors.margin_bottom && (
                <div style={{ marginTop: '4px', fontSize: '12px', color: '#d32f2f' }}>
                  {errors.margin_bottom.message}
                </div>
              )}
            </div>
          </div>
        </div>

        {/* Submit Button */}
        <button
          type="submit"
          disabled={!isValid || isSubmitting}
          style={{
            width: '100%',
            padding: '12px',
            backgroundColor: !isValid || isSubmitting ? '#ccc' : '#1976d2',
            color: '#fff',
            border: 'none',
            borderRadius: '4px',
            fontSize: '16px',
            fontWeight: 500,
            cursor: !isValid || isSubmitting ? 'not-allowed' : 'pointer',
          }}
        >
          {isSubmitting ? 'Đang lưu...' : 'Lưu cấu hình'}
        </button>
      </form>
    </div>
  );
};
