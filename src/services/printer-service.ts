import { invoke } from "@tauri-apps/api/core";

import type { Printer, PrinterConfig, PrinterStatus } from "../types";

export interface MetricsDto {
  total_jobs: number;
  completed: number;
  failed: number;
  last_print_time_secs: number;
}

export async function listPrinters(): Promise<Printer[]> {
  return invoke<Printer[]>("list_printers");
}

export async function savePrinterConfig(config: PrinterConfig): Promise<void> {
  return invoke("save_printer_config", { config });
}

export async function getPrinterConfig(): Promise<PrinterConfig> {
  return invoke<PrinterConfig>("get_printer_config");
}

export async function getPrinterStatus(name: string): Promise<PrinterStatus> {
  return invoke<PrinterStatus>("get_printer_status", { name });
}

export async function getMetrics(): Promise<MetricsDto> {
  return invoke<MetricsDto>("get_metrics");
}

export async function clearJobHistory(): Promise<number> {
  return invoke<number>("clear_job_history");
}

export async function getAutostartEnabled(): Promise<boolean> {
  return invoke<boolean>("get_autostart_enabled");
}

export async function setAutostartEnabled(enabled: boolean): Promise<void> {
  return invoke("set_autostart_enabled", { enabled });
}
