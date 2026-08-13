import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";

interface PrinterCategoryResult {
  category: "pdf" | "virtual" | "physical";
  needs_rendering: boolean;
  needs_save_dialog: boolean;
  description: string;
}

export async function detectPrinterCategory(printerName: string): Promise<PrinterCategoryResult> {
  return invoke("detect_printer_category", { printerName });
}

export async function showPrintToFileDialog(): Promise<string | null> {
  const timestamp = Date.now();
  const defaultFilename = `SAPO_Print_${timestamp}.pdf`;

  const filePath = await save({
    defaultPath: defaultFilename,
    filters: [
      {
        name: "PDF Document",
        extensions: ["pdf"],
      },
    ],
    title: "Lưu file PDF",
  });

  return filePath;
}
