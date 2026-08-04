import {Box, Button, Checkbox, FormLayout, Select2, TextField} from '@sapo/ui-components';
import styled from '@emotion/styled';
import * as yup from 'yup';
import {Controller, useForm} from 'react-hook-form';
import {yupResolver} from '@hookform/resolvers/yup';
import {useMemo, useState, useEffect} from 'react';
import {ConfirmModal} from '../../../components/ConfirmModal';
import {DefaultPrintConfig, ImageFormatOptions, PaperSizeOptions} from '../../../constants/printer';
import {getDimensionsByPaperSize} from '../../../constants/printer';
import {listPrinters, savePrinterConfig, getPrinterConfig} from '../../../services/printer-service';
import {PrinterDto, PrinterConfigDto} from '../../../types';
import {showToast, showErrorToast} from '../../../utils/toast';

const validationSchema = yup.object().shape({
  printerName: yup.string().required('Tên máy in không được để trống'),
  paperSize: yup.string().required('Khổ giấy không được để trống'),
  width: yup
    .number()
    .nullable()
    .transform((value, original) => (original === '' ? null : value))
    .typeError('Phải là số')
    .when('paperSize', {
      is: 'Custom',
      then: (schema) =>
        schema
          .required('Chiều rộng bắt buộc khi chọn khổ Custom')
          .min(50, 'Chiều rộng phải trong khoảng 50-500mm')
          .max(500, 'Chiều rộng phải trong khoảng 50-500mm'),
      otherwise: (schema) => schema.defined(),
    }),
  height: yup
    .number()
    .nullable()
    .transform((value, original) => (original === '' ? null : value))
    .typeError('Phải là số')
    .when('paperSize', {
      is: 'Custom',
      then: (schema) =>
        schema
          .required('Chiều cao bắt buộc khi chọn khổ Custom')
          .min(50, 'Chiều cao phải trong khoảng 50-500mm')
          .max(500, 'Chiều cao phải trong khoảng 50-500mm'),
      otherwise: (schema) => schema.defined(),
    }),
  marginLeft: yup
    .number()
    .typeError('Phải là số')
    .required('Lề trái không được để trống')
    .min(0, 'Lề trái phải trong khoảng 0-100mm')
    .max(100, 'Lề trái phải trong khoảng 0-100mm'),
  marginRight: yup
    .number()
    .typeError('Phải là số')
    .required('Lề phải không được để trống')
    .min(0, 'Lề phải phải trong khoảng 0-100mm')
    .max(100, 'Lề phải phải trong khoảng 0-100mm'),
  marginTop: yup
    .number()
    .typeError('Phải là số')
    .required('Lề trên không được để trống')
    .min(0, 'Lề trên phải trong khoảng 0-100mm')
    .max(100, 'Lề trên phải trong khoảng 0-100mm'),
  marginBottom: yup
    .number()
    .typeError('Phải là số')
    .required('Lề dưới không được để trống')
    .min(0, 'Lề dưới phải trong khoảng 0-100mm')
    .max(100, 'Lề dưới phải trong khoảng 0-100mm'),
  landscape: yup.boolean().required(),
  printingBuffer: yup.boolean().required(),
  printImage: yup.boolean().required(),
  imageFormat: yup
    .string()
    .required()
    .when('printImage', {
      is: true,
      then: (schema) =>
        schema.oneOf(['RGB', 'ARGB', 'BGR', 'GRAY', 'BINARY'], 'Loại ảnh in không hợp lệ'),
    }),
});

type PrinterSettingsFormData = yup.InferType<typeof validationSchema>;

interface PrinterSettingsFormProps {
  onSaved?: () => void;
  onCancel?: () => void;
}

const PrinterSettingsForm = ({onSaved, onCancel}: PrinterSettingsFormProps) => {
  const [printers, setPrinters] = useState<PrinterDto[]>([]);
  const [loadingPrinters, setLoadingPrinters] = useState(true);
  const [modalName, setModalName] = useState<'cancel' | 'restore'>();
  const closeModal = () => setModalName(undefined);

  const {
    control,
    handleSubmit,
    watch,
    setValue,
    reset,
    formState: {errors},
  } = useForm<PrinterSettingsFormData>({
    resolver: yupResolver(validationSchema) as any,
    defaultValues: DefaultPrintConfig,
  });

  useEffect(() => {
    // Load printers
    listPrinters()
      .then((result) => {
        setPrinters(result);
        const defaultPrinter = result.find((p) => p.is_default) || result[0];
        if (defaultPrinter) {
          setValue('printerName', defaultPrinter.name);
        }
      })
      .catch((err) => {
        console.error('Failed to load printers:', err);
      })
      .finally(() => setLoadingPrinters(false));

    // Load existing config
    getPrinterConfig()
      .then((config) => {
        if (config && config.printer_name) {
          reset({
            printerName: config.printer_name,
            paperSize: config.paper_size,
            width: config.paper_width,
            height: config.paper_height,
            marginLeft: config.margin_left,
            marginRight: config.margin_right,
            marginTop: config.margin_top,
            marginBottom: config.margin_bottom,
            landscape: config.orientation === 'Landscape',
            printingBuffer: config.enable_buffer,
            printImage: config.print_as_image,
            imageFormat: config.color_mode,
          });
        }
      })
      .catch((err) => {
        console.error('Failed to load config:', err);
      });
  }, [setValue, reset]);

  const printerOptions = useMemo(() => {
    if (loadingPrinters) {
      return [{label: 'Đang tải...', value: '', disabled: true}];
    }
    if (printers.length === 0) {
      return [{label: 'Không tìm thấy máy in', value: '', disabled: true}];
    }
    return printers.map((printer) => ({
      label: `${printer.name}${printer.is_default ? ' (Mặc định)' : ''}${printer.status !== 'Online' ? ` - ${printer.status}` : ''}`,
      value: printer.name,
    }));
  }, [printers, loadingPrinters]);

  const paperSize = watch('paperSize');
  const isCustomPaperSize = paperSize === 'Custom';
  const printImage = watch('printImage');

  const handlePaperSizeChange = (value: string) => {
    setValue('paperSize', value);
    const dimensions = getDimensionsByPaperSize(value);
    if (dimensions) {
      setValue('width', parseFloat(dimensions.width));
      setValue('height', parseFloat(dimensions.height));
    } else if (value === 'Custom') {
      // Clear values for custom size
      setValue('width', undefined);
      setValue('height', undefined);
    }
  };

  const onSubmit = async (data: PrinterSettingsFormData) => {
    try {
      const config: PrinterConfigDto = {
        printer_name: data.printerName,
        paper_size: data.paperSize,
        paper_width: data.width ?? undefined,
        paper_height: data.height ?? undefined,
        orientation: data.landscape ? 'Landscape' : 'Portrait',
        margin_left: data.marginLeft,
        margin_right: data.marginRight,
        margin_top: data.marginTop,
        margin_bottom: data.marginBottom,
        print_as_image: data.printImage,
        color_mode: data.printImage ? data.imageFormat : 'RGB',
        enable_buffer: data.printingBuffer,
        buffer_size_kb: undefined,
      };
      await savePrinterConfig(config);
      showToast('Đã lưu cấu hình máy in');

      // Wait a bit for file write to complete before callback
      setTimeout(() => {
        onSaved?.();
      }, 100);
    } catch (err) {
      console.error('Save config error:', err);
      showErrorToast(err as string);
    }
  };

  const handleCancel = () => setModalName('cancel');

  const handleConfirmCancel = () => {
    closeModal();
    onCancel?.();
  };

  const handleRestore = () => setModalName('restore');

  const handleConfirmRestore = () => {
    reset({...DefaultPrintConfig});
    closeModal();
  };

  const cancelConfirmModal = modalName === "cancel" && (
      <ConfirmModal
          open
          title="Hủy chỉnh sửa"
          body="Thông tin thay đổi của bạn sẽ mất. Bạn có xác nhận thay đổi?"
          onDismiss={closeModal}
          confirmAction={{
            content: 'Xác nhận',
            onAction: handleConfirmCancel,
          }}
      />
  );

  const restoreConfirmModal = modalName === "restore" && (
      <ConfirmModal
          open
          title="Khôi phục cài đặt"
          body="Bạn có xác nhận thiết lập lại cài đặt về mặc định không?"
          onDismiss={closeModal}
          confirmAction={{
            content: 'Xác nhận',
            onAction: handleConfirmRestore,
          }}
      />
  );

  return (
    <>
      <Box>
        <ButtonGroupStyled>
          <Button plain onClick={handleSubmit(onSubmit)}>
            Lưu
          </Button>
          <Button plain destructive onClick={handleCancel}>
            Hủy
          </Button>
          <Button plain onClick={handleRestore}>
            Khôi phục cài đặt
          </Button>
        </ButtonGroupStyled>

        <Box padding="4">
          <FormLayout>
            <FormLayout.Group>
              <Controller
                name="printerName"
                control={control}
                render={({field}) => (
                  <Select2
                    label="Máy in"
                    value={field.value}
                    onChange={field.onChange}
                    options={printerOptions}
                    disabled={loadingPrinters}
                  />
                )}
              />
            </FormLayout.Group>

            <FormLayout.Group>
              <Controller
                name="paperSize"
                control={control}
                render={({field}) => (
                  <Select2
                    label="Khổ giấy"
                    value={field.value}
                    onChange={handlePaperSizeChange}
                    options={[...PaperSizeOptions]}
                  />
                )}
              />

              <Controller
                name="width"
                control={control}
                render={({field}) => (
                  <TextField
                    label="Chiều rộng"
                    type="number"
                    value={field.value?.toString() || ''}
                    onChange={field.onChange}
                    suffix="mm"
                    disabled={!isCustomPaperSize}
                    error={errors.width?.message}
                  />
                )}
              />

              <Controller
                name="height"
                control={control}
                render={({field}) => (
                  <TextField
                    label="Chiều cao"
                    type="number"
                    value={field.value?.toString() || ''}
                    onChange={field.onChange}
                    suffix="mm"
                    disabled={!isCustomPaperSize}
                    error={errors.height?.message}
                  />
                )}
              />
            </FormLayout.Group>

            <FormLayout.Group>
              <Controller
                name="marginLeft"
                control={control}
                render={({field}) => (
                  <TextField
                    label="Cân lề trái"
                    type="number"
                    value={field.value?.toString() || '0'}
                    onChange={field.onChange}
                    suffix="mm"
                    error={errors.marginLeft?.message}
                  />
                )}
              />

              <Controller
                name="marginRight"
                control={control}
                render={({field}) => (
                  <TextField
                    label="Cân lề phải"
                    type="number"
                    value={field.value?.toString() || '0'}
                    onChange={field.onChange}
                    suffix="mm"
                    error={errors.marginRight?.message}
                  />
                )}
              />
            </FormLayout.Group>

            <FormLayout.Group>
              <Controller
                name="marginTop"
                control={control}
                render={({field}) => (
                  <TextField
                    label="Cân lề trên"
                    type="number"
                    value={field.value?.toString() || '0'}
                    onChange={field.onChange}
                    suffix="mm"
                    error={errors.marginTop?.message}
                  />
                )}
              />

              <Controller
                name="marginBottom"
                control={control}
                render={({field}) => (
                  <TextField
                    label="Cân lề dưới"
                    type="number"
                    value={field.value?.toString() || '0'}
                    onChange={field.onChange}
                    suffix="mm"
                    error={errors.marginBottom?.message}
                  />
                )}
              />
            </FormLayout.Group>

            <FormLayout.Group>
              <Controller
                name="landscape"
                control={control}
                render={({field}) => (
                  <Checkbox checked={field.value} onChange={field.onChange} label="In chiều ngang" />
                )}
              />
              <Controller
                name="printingBuffer"
                control={control}
                render={({field}) => (
                  <Checkbox checked={field.value} onChange={field.onChange} label="Printing Buffer" />
                )}
              />
              <Controller
                name="printImage"
                control={control}
                render={({field}) => (
                  <Checkbox checked={field.value} onChange={field.onChange} label="In ảnh" />
                )}
              />
            </FormLayout.Group>

            {printImage && (
              <Controller
                name="imageFormat"
                control={control}
                render={({field}) => (
                  <Select2
                    label="Loại ảnh in"
                    value={field.value}
                    onChange={field.onChange}
                    options={[...ImageFormatOptions]}
                  />
                )}
              />
            )}
          </FormLayout>
        </Box>
      </Box>
      {cancelConfirmModal}
      {restoreConfirmModal}
    </>
  );
};

const ButtonGroupStyled = styled.div`
  display: flex;
  justify-content: start;
  gap: ${(p) => p.theme.spacing('8')};
  padding: ${(p) => p.theme.spacing('4')};
  background-color: #f2f9ff;
`;

export default PrinterSettingsForm;
