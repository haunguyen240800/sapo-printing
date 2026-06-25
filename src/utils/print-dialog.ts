import { save } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';

/**
 * Printer category detection result from backend
 */
interface PrinterCategoryResult {
  category: 'pdf' | 'virtual' | 'physical';
  needs_rendering: boolean;
  needs_save_dialog: boolean;
  description: string;
}

/**
 * Detect printer category using backend classification logic.
 * This ensures frontend and backend use the same printer detection rules.
 */
export async function detectPrinterCategory(
  printerName: string
): Promise<PrinterCategoryResult> {
  return invoke('detect_printer_category', { printerName });
}

/**
 * Show native "Save As" dialog for Print to PDF printers.
 * Returns the selected file path or null if user cancelled.
 */
export async function showPrintToFileDialog(): Promise<string | null> {
  const timestamp = Date.now();
  const defaultFilename = `SAPO_Print_${timestamp}.pdf`;

  const filePath = await save({
    defaultPath: defaultFilename,
    filters: [
      {
        name: 'PDF Document',
        extensions: ['pdf'],
      },
    ],
    title: 'Lưu file PDF',
  });

  return filePath;
}

/**
 * Check if printer is a "Print to PDF" virtual printer (saves to file, no rendering needed)
 * @deprecated Use detectPrinterCategory() instead for consistent backend logic
 */
export function isPrintToPdfPrinter(printerName: string): boolean {
  const pdfPrinterPatterns = [
    'Microsoft Print to PDF',
    'Print to PDF',
    'Save as PDF',
    'PDF Printer',
    'Adobe PDF',
    'Foxit Reader PDF Printer',
    'Nitro PDF Creator',
    'CutePDF Writer',
    'doPDF',
  ];

  return pdfPrinterPatterns.some((pattern) =>
    printerName.toLowerCase().includes(pattern.toLowerCase())
  );
}

/**
 * Check if printer is a virtual/file printer (XPS, Image, etc.)
 * These printers also don't need bitmap rendering
 * @deprecated Use detectPrinterCategory() instead for consistent backend logic
 */
export function isVirtualPrinter(printerName: string): boolean {
  const virtualPrinterPatterns = [
    'Microsoft XPS Document Writer',
    'Microsoft Print to Image',
    'Fax',
    'OneNote',
    'Send to OneNote',
  ];

  return virtualPrinterPatterns.some((pattern) =>
    printerName.toLowerCase().includes(pattern.toLowerCase())
  );
}

/**
 * Check if printer needs bitmap rendering (real physical/network printers)
 * @deprecated Use detectPrinterCategory() instead for consistent backend logic
 */
export function needsRenderingToBitmap(printerName: string): boolean {
  // If it's a virtual/PDF printer, no rendering needed
  if (isPrintToPdfPrinter(printerName) || isVirtualPrinter(printerName)) {
    return false;
  }

  // Otherwise, it's a real printer that needs bitmap
  return true;
}

/**
 * Get printer type for logging/debugging
 * @deprecated Use detectPrinterCategory() instead for consistent backend logic
 */
export function getPrinterType(printerName: string): 'pdf' | 'virtual' | 'physical' {
  if (isPrintToPdfPrinter(printerName)) return 'pdf';
  if (isVirtualPrinter(printerName)) return 'virtual';
  return 'physical';
}

