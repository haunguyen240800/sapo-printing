// Shared TypeScript types for printer-related DTOs
// These match the Rust DTOs defined in src-tauri/src/interface/tauri/dtos/printer_dto.rs

export interface PrinterDto {
  name: string;
  device_id: string;
  status: string;
  printer_type: string;
  is_default?: boolean;
}

export interface PrinterConfigDto {
  printer_name?: string;
  paper_size?: string;
  paper_width?: number;
  paper_height?: number;
  orientation?: string;
  margin_left?: number;
  margin_right?: number;
  margin_top?: number;
  margin_bottom?: number;
  print_as_image?: boolean;
  color_mode?: string;
  enable_buffer?: boolean;
  buffer_size_kb?: number;
}

export interface PrinterStatusDto {
  status: string;
}

// Form data types
export type PaperSizeValue = 'A4' | 'A5' | 'Letter' | 'CM10x10' | 'CM10x12' | 'CM10x15' | 'CM10x18' | 'Custom';

export interface PrintConfigFormData {
  printerName: string;
  paperSize: string;
  width: number | null | undefined;
  height: number | null | undefined;
  marginLeft: number;
  marginRight: number;
  marginTop: number;
  marginBottom: number;
  landscape: boolean;
  printingBuffer: boolean;
  printImage: boolean;
  imageFormat: string;
}
