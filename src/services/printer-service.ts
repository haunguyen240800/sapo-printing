import { invoke } from "@tauri-apps/api/core";

import type { PrinterConfigDto, PrinterDto, PrinterStatusDto } from "../types";
import type { JobDto, JobFilterDto } from "../types/print-job";
import { detectPrinterCategory, showPrintToFileDialog } from "../utils/print-dialog";

export interface MetricsDto {
  collected_at: number;
  job_metrics: {
    total_jobs: number;
    pending: number;
    queued: number;
    downloaded: number;
    submitted: number;
    printing: number;
    completed: number;
    failed: number;
    cancelled: number;
    success_rate: number;
  };
  queue_metrics: {
    current_depth: number;
    avg_wait_time_secs: number;
  };
  printer_metrics: {
    printers: {
      printer_name: string;
      total_jobs: number;
      completed_jobs: number;
      utilization_percent: number;
    }[];
  };
  performance_metrics: {
    avg_job_duration_secs: number;
    p50_job_duration_secs: number;
    p95_job_duration_secs: number;
    p99_job_duration_secs: number;
    avg_download_time_secs: number;
    avg_render_time_secs: number;
    avg_print_time_secs: number;
  };
}

export async function listPrinters(): Promise<PrinterDto[]> {
  return invoke<PrinterDto[]>("list_printers");
}

export async function savePrinterConfig(config: PrinterConfigDto): Promise<void> {
  return invoke("save_printer_config", { config });
}

export async function getPrinterConfig(): Promise<PrinterConfigDto> {
  return invoke<PrinterConfigDto>("get_printer_config");
}

export async function getPrinterStatus(name: string): Promise<PrinterStatusDto> {
  return invoke<PrinterStatusDto>("get_printer_status", { name });
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

export async function listJobs(filter: JobFilterDto): Promise<JobDto[]> {
  return invoke<JobDto[]>("list_jobs", { filter });
}

export async function getJobStatus(jobId: string): Promise<JobDto> {
  return invoke<JobDto>("get_job_status", { job_id: jobId });
}

export async function getMetrics(): Promise<MetricsDto> {
  return invoke<MetricsDto>("get_metrics");
}
