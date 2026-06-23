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

export interface PrinterStatusDto {
  status: string;
}
