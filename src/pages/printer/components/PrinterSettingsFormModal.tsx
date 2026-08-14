import { useEffect, useMemo, useState } from "react";
import { Controller, useForm } from "react-hook-form";
import { yupResolver } from "@hookform/resolvers/yup";
import { Checkbox, FormLayout, Icon, InlineStack, Modal, NumberField, Select2, Text } from "@sapo/ui-components";
import { WarningIcon } from "@sapo/ui-icons";
import { ConfirmModal } from "src/components/ConfirmModal";
import {
  DefaultPrintConfig,
  getDimensionsByPaperSize,
  ImageFormatOptions,
  PaperSizeOptions,
} from "src/constants/printer";
import { getPrinterConfig, listPrinters, savePrinterConfig } from "src/services/printer-service";
import { Printer, PrinterConfig } from "src/types";
import { showErrorToast, showToast } from "src/utils/toast";
import * as yup from "yup";

const validationSchema = yup.object().shape({
  printerName: yup.string().required("Tên máy in không được để trống"),
  paperSize: yup.string().required("Khổ giấy không được để trống"),
  width: yup
    .number()
    .nullable()
    .transform((value, original) => (original === "" ? null : value))
    .when("paperSize", {
      is: "Custom",
      then: (schema) =>
        schema
          .required("Chiều rộng bắt buộc khi chọn khổ Custom")
          .min(50, "Chiều rộng phải tối thiểu 50mm"),
      otherwise: (schema) => schema.defined(),
    }),
  height: yup
    .number()
    .nullable()
    .transform((value, original) => (original === "" ? null : value))
    .when("paperSize", {
      is: "Custom",
      then: (schema) =>
        schema
          .required("Chiều cao bắt buộc khi chọn khổ Custom")
          .min(50, "Chiều cao phải tối thiểu 50mm"),
      otherwise: (schema) => schema.defined(),
    }),
  marginLeft: yup
    .number()
    .transform((value, original) => (original === "" ? 0 : value))
    .required("Lề trái không được để trống")
    .min(0, "Lề trái không được âm"),
  marginRight: yup
    .number()
    .transform((value, original) => (original === "" ? 0 : value))
    .required("Lề phải không được để trống")
    .min(0, "Lề phải không được âm"),
  marginTop: yup
    .number()
    .transform((value, original) => (original === "" ? 0 : value))
    .required("Lề trên không được để trống")
    .min(0, "Lề trên không được âm"),
  marginBottom: yup
    .number()
    .transform((value, original) => (original === "" ? 0 : value))
    .required("Lề dưới không được để trống")
    .min(0, "Lề dưới không được âm"),
  landscape: yup.boolean().required(),
  printingBuffer: yup.boolean().required(),
  printImage: yup.boolean().required(),
  imageFormat: yup
    .string()
    .required()
    .when("printImage", {
      is: true,
      then: (schema) => schema.oneOf(["RGB", "ARGB", "BGR", "GRAY", "BINARY"], "Loại ảnh in không hợp lệ"),
    }),
});

type PrinterSettingsFormData = yup.InferType<typeof validationSchema>;

interface PrinterSettingsFormProps {
  open: boolean;
  onClose: () => void;
  onSaved?: () => void;
}

const PrinterSettingsFormModal = ({ open, onClose, onSaved }: PrinterSettingsFormProps) => {
  const [printers, setPrinters] = useState<Printer[]>([]);
  const [loadingPrinters, setLoadingPrinters] = useState(true);
  const [modalName, setModalName] = useState<"cancel" | "restore">();
  const closeModal = () => setModalName(undefined);

  const {
    control,
    handleSubmit,
    watch,
    setValue,
    reset,
    formState: { errors, isDirty },
  } = useForm<PrinterSettingsFormData>({
    resolver: yupResolver(validationSchema) as any,
    defaultValues: DefaultPrintConfig,
  });

  useEffect(() => {
    // Load printers
    const loadPrinters = async () => {
      try {
        const result = await listPrinters();
        setPrinters(result);
        const defaultPrinter = result.find((p) => p.is_default) || result[0];
        if (defaultPrinter) {
          setValue("printerName", defaultPrinter.name);
        }
      } catch {
        showErrorToast("Không tải được danh sách máy in");
      } finally {
        setLoadingPrinters(false);
      }
    };

    // Load existing config
    const loadConfig = async () => {
      try {
        const config = await getPrinterConfig();
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
            landscape: config.orientation === "Landscape",
            printingBuffer: config.enable_buffer,
            printImage: config.print_as_image,
            imageFormat: config.color_mode,
          });
        }
      } catch {
        showErrorToast("Không tải được cấu hình máy in");
      }
    };

    loadPrinters();
    loadConfig();
  }, [setValue, reset]);

  const printerOptions = useMemo(() => {
    if (loadingPrinters) {
      return [{ label: "Đang tải...", value: "", disabled: true }];
    }
    if (printers.length === 0) {
      return [{ label: "Không tìm thấy máy in", value: "", disabled: true }];
    }
    return printers.map((printer) => ({
      label: `${printer.name}${printer.is_default ? " (Mặc định)" : ""}${printer.status !== "Online" ? ` - ${printer.status}` : ""}`,
      value: printer.name,
    }));
  }, [printers, loadingPrinters]);

  const paperSize = watch("paperSize");
  const isCustomPaperSize = paperSize === "Custom";
  const printImage = watch("printImage");

  const handlePaperSizeChange = (value: string) => {
    setValue("paperSize", value, { shouldDirty: true });
    const dimensions = getDimensionsByPaperSize(value);
    if (dimensions) {
      setValue("width", parseFloat(dimensions.width), { shouldDirty: true });
      setValue("height", parseFloat(dimensions.height), { shouldDirty: true });
    } else if (value === "Custom") {
      setValue("width", undefined, { shouldDirty: true });
      setValue("height", undefined, { shouldDirty: true });
    }
  };

  const onSubmit = async (data: PrinterSettingsFormData) => {
    try {
      const config: PrinterConfig = {
        printer_name: data.printerName,
        paper_size: data.paperSize,
        paper_width: data.width ?? undefined,
        paper_height: data.height ?? undefined,
        orientation: data.landscape ? "Landscape" : "Portrait",
        margin_left: data.marginLeft,
        margin_right: data.marginRight,
        margin_top: data.marginTop,
        margin_bottom: data.marginBottom,
        print_as_image: data.printImage,
        color_mode: data.printImage ? data.imageFormat : "RGB",
        enable_buffer: data.printingBuffer,
        buffer_size_kb: undefined,
      };
      await savePrinterConfig(config);
      showToast("Đã lưu cấu hình máy in");

      setTimeout(() => {
        onSaved?.();
        onClose();
      }, 100);
    } catch (err) {
      showErrorToast(err as string);
    }
  };

  const handleCancel = () => {
    if (isDirty) {
      setModalName("cancel");
    } else {
      onClose();
    }
  };

  const handleConfirmCancel = () => {
    closeModal();
    onClose();
  };

  const handleRestore = () => setModalName("restore");

  const handleConfirmRestore = () => {
    reset({ ...DefaultPrintConfig });
    closeModal();
  };

  const cancelConfirmModal = modalName === "cancel" && (
    <ConfirmModal
      open
      title={
        <InlineStack gap="2" blockAlign="center">
          <Text as="span" variant="headingLg"><Icon source={WarningIcon} tone="warning" /></Text>
          <Text as="span" variant="headingLg">Hủy chỉnh sửa?</Text>
        </InlineStack>
      }
      body="Thông tin thay đổi của bạn sẽ mất. Bạn có xác nhận thay đổi?"
      onDismiss={closeModal}
      confirmAction={{
        content: "Xác nhận",
        onAction: handleConfirmCancel,
      }}
    />
  );

  const restoreConfirmModal = modalName === "restore" && (
    <ConfirmModal
      open
      title={
        <InlineStack gap="2" blockAlign="center">
          <Text as="span" variant="headingLg"><Icon source={WarningIcon} tone="warning" /></Text>
          <Text as="span" variant="headingLg">Khôi phục cài đặt?</Text>
        </InlineStack>
      }
      body="Bạn có xác nhận thiết lập lại cài đặt về mặc định không?"
      onDismiss={closeModal}
      confirmAction={{
        content: "Xác nhận",
        onAction: handleConfirmRestore,
      }}
    />
  );

  return (
    <>
      <Modal
        open={open}
        onClose={handleCancel}
        title="Cấu hình in"
        size="large"
        sectioned
        primaryAction={{
          content: "Lưu",
          onAction: handleSubmit(onSubmit),
          disabled: !isDirty,
        }}
        secondaryActions={[
          {
            content: "Khôi phục cài đặt",
            outline: true,
            onAction: handleRestore,
          },
          {
            content: "Hủy",
            destructive: true,
            onAction: handleCancel,
          },
        ]}
      >
        <FormLayout>
          <FormLayout.Group>
            <Controller
              name="printerName"
              control={control}
              render={({ field }) => (
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
              render={({ field }) => (
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
              render={({ field }) => (
                <NumberField
                  label="Chiều rộng"
                  value={field.value ?? undefined}
                  onChange={(val?: number) => field.onChange(val)}
                  suffix="mm"
                  min={0}
                  allowNegative={false}
                  disabled={!isCustomPaperSize}
                  error={errors.width?.message}
                />
              )}
            />

            <Controller
              name="height"
              control={control}
              render={({ field }) => (
                <NumberField
                  label="Chiều cao"
                  value={field.value ?? undefined}
                  onChange={(val?: number) => field.onChange(val)}
                  suffix="mm"
                  min={0}
                  allowNegative={false}
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
              render={({ field }) => (
                <NumberField
                  label="Căn lề trái"
                  value={field.value}
                  onChange={(val?: number) => field.onChange(val ?? 0)}
                  suffix="mm"
                  min={0}
                  allowNegative={false}
                  error={errors.marginLeft?.message}
                />
              )}
            />

            <Controller
              name="marginRight"
              control={control}
              render={({ field }) => (
                <NumberField
                  label="Căn lề phải"
                  value={field.value}
                  onChange={(val?: number) => field.onChange(val ?? 0)}
                  suffix="mm"
                  min={0}
                  allowNegative={false}
                  error={errors.marginRight?.message}
                />
              )}
            />
          </FormLayout.Group>

          <FormLayout.Group>
            <Controller
              name="marginTop"
              control={control}
              render={({ field }) => (
                <NumberField
                  label="Căn lề trên"
                  value={field.value}
                  onChange={(val: number | undefined) => field.onChange(val ?? 0)}
                  suffix="mm"
                  min={0}
                  allowNegative={false}
                  error={errors.marginTop?.message}
                />
              )}
            />

            <Controller
              name="marginBottom"
              control={control}
              render={({ field }) => (
                <NumberField
                  label="Căn lề dưới"
                  value={field.value}
                  onChange={(val: number | undefined) => field.onChange(val ?? 0)}
                  suffix="mm"
                  min={0}
                  allowNegative={false}
                  error={errors.marginBottom?.message}
                />
              )}
            />
          </FormLayout.Group>

          <FormLayout.Group>
            <Controller
              name="landscape"
              control={control}
              render={({ field }) => (
                <Checkbox checked={field.value} onChange={field.onChange} label="In chiều ngang" />
              )}
            />
            <Controller
              name="printingBuffer"
              control={control}
              render={({ field }) => (
                <Checkbox checked={field.value} onChange={field.onChange} label="Printing Buffer" />
              )}
            />
            <Controller
              name="printImage"
              control={control}
              render={({ field }) => <Checkbox checked={field.value} onChange={field.onChange} label="In ảnh" />}
            />
          </FormLayout.Group>

          {printImage && (
            <FormLayout.Group>
              <Controller
                name="imageFormat"
                control={control}
                render={({ field }) => (
                  <Select2
                    label="Loại ảnh in"
                    value={field.value}
                    onChange={field.onChange}
                    options={[...ImageFormatOptions]}
                  />
                )}
              />
              <></>
            </FormLayout.Group>
          )}
        </FormLayout>
      </Modal>
      {cancelConfirmModal}
      {restoreConfirmModal}
    </>
  );
};

export default PrinterSettingsFormModal;
