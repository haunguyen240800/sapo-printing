import { invoke } from "@tauri-apps/api/core";

import type { Printer, PrinterConfig, PrinterStatus } from "../types";
import type { Job, JobFilter } from "../types/print-job";
import { detectPrinterCategory, showPrintToFileDialog } from "../utils/print-dialog";

export interface MetricsDto {
  total_jobs: number;
  completed: number;
  failed: number;
  avg_print_time_secs: number;
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

export async function createPrintJob(pdfUrls: string[], printerName: string): Promise<string[]> {
  let outputPath: string | null = null;

  // Use backend printer category detection for consistent logic
  const printerCategory = await detectPrinterCategory(printerName);

  // If printing to PDF, show native "Save As" dialog
  if (printerCategory.needs_save_dialog) {
    outputPath = await showPrintToFileDialog();

    // User cancelled the dialog
    if (!outputPath) {
      throw new Error("User cancelled file selection");
    }
  }

  return invoke<string[]>("create_print_job", {
    payload: {
      pdf_urls: pdfUrls,
      printer_name: printerName,
      output_path: outputPath,
    },
  });
}

export async function cancelPrintJob(jobId: string): Promise<void> {
  return invoke("cancel_print_job", {
    payload: { job_id: jobId },
  });
}

export async function listJobs(filter: JobFilter): Promise<Job[]> {
  return invoke<Job[]>("list_jobs", { filter });
}

export async function getJobStatus(jobId: string): Promise<Job> {
  return invoke<Job>("get_job_status", { job_id: jobId });
}

export async function getMetrics(): Promise<MetricsDto> {
  return invoke<MetricsDto>("get_metrics");
}

export async function getAutostartEnabled(): Promise<boolean> {
  return invoke<boolean>("get_autostart_enabled");
}

export async function setAutostartEnabled(enabled: boolean): Promise<void> {
  return invoke("set_autostart_enabled", { enabled });
}
