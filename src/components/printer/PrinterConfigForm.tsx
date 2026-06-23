import React, { useEffect, useState } from 'react';
import { useForm, Controller } from 'react-hook-form';
import { yupResolver } from '@hookform/resolvers/yup';
import * as yup from 'yup';
import { invoke } from '@tauri-apps/api/core';
import { TextField, Select, Checkbox, Button, Banner } from '@sapo/ui-components';
import { PrinterSelector } from './PrinterSelector';
import { PrinterStatus } from './PrinterStatus';
import { PrinterDto } from '../../types/printer';

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
    handleSubmit,
    watch,
    setValue,
    control,
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
        <Banner
          status={notification.type === 'success' ? 'success' : 'critical'}
          onDismiss={() => setNotification(null)}
        >
          {notification.message}
        </Banner>
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
            <Controller
              name="paper_size"
              control={control}
              render={({ field }) => (
                <Select
                  label="Khổ giấy"
                  options={[
                    { label: 'A4 (210 × 297 mm)', value: 'A4' },
                    { label: 'A5 (148 × 210 mm)', value: 'A5' },
                    { label: 'Letter (216 × 279 mm)', value: 'Letter' },
                    { label: 'Tùy chỉnh', value: 'Custom' },
                  ]}
                  value={field.value}
                  onChange={field.onChange}
                  error={errors.paper_size?.message}
                />
              )}
            />
          </div>

          {selectedPaperSize === 'Custom' && (
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px', marginBottom: '16px' }}>
              <Controller
                name="paper_width"
                control={control}
                render={({ field }) => (
                  <TextField
                    label="Chiều rộng (mm)"
                    type="number"
                    value={field.value?.toString() || ''}
                    onChange={field.onChange}
                    error={errors.paper_width?.message}
                  />
                )}
              />
              <Controller
                name="paper_height"
                control={control}
                render={({ field }) => (
                  <TextField
                    label="Chiều cao (mm)"
                    type="number"
                    value={field.value?.toString() || ''}
                    onChange={field.onChange}
                    error={errors.paper_height?.message}
                  />
                )}
              />
            </div>
          )}

          <div>
            <Checkbox
              label="In chiều ngang"
              checked={watch('orientation') === 'Landscape'}
              onChange={(checked) => setValue('orientation', checked ? 'Landscape' : 'Portrait')}
            />
          </div>
        </div>

        {/* Section 3: Layout - Margins */}
        <div style={{ marginBottom: '24px', padding: '16px', border: '1px solid #e0e0e0', borderRadius: '4px' }}>
          <h2 style={{ marginBottom: '16px', fontSize: '18px' }}>3. Lề trang</h2>

          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '16px' }}>
            <Controller
              name="margin_left"
              control={control}
              render={({ field }) => (
                <TextField
                  label="Lề trái (mm)"
                  type="number"
                  value={field.value?.toString() || '0'}
                  onChange={field.onChange}
                  error={errors.margin_left?.message}
                />
              )}
            />
            <Controller
              name="margin_right"
              control={control}
              render={({ field }) => (
                <TextField
                  label="Lề phải (mm)"
                  type="number"
                  value={field.value?.toString() || '0'}
                  onChange={field.onChange}
                  error={errors.margin_right?.message}
                />
              )}
            />
            <Controller
              name="margin_top"
              control={control}
              render={({ field }) => (
                <TextField
                  label="Lề trên (mm)"
                  type="number"
                  value={field.value?.toString() || '0'}
                  onChange={field.onChange}
                  error={errors.margin_top?.message}
                />
              )}
            />
            <Controller
              name="margin_bottom"
              control={control}
              render={({ field }) => (
                <TextField
                  label="Lề dưới (mm)"
                  type="number"
                  value={field.value?.toString() || '0'}
                  onChange={field.onChange}
                  error={errors.margin_bottom?.message}
                />
              )}
            />
          </div>
        </div>

        {/* Submit Button */}
        <Button
          submit
          primary
          disabled={!isValid || isSubmitting}
          loading={isSubmitting}
          fullWidth
        >
          Lưu cấu hình
        </Button>
      </form>
    </div>
  );
};
