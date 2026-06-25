#[cfg(target_os = "windows")]
use super::super::printer_engine::PrinterEngine;
#[cfg(target_os = "windows")]
use crate::shared::errors::InfrastructureError;
#[cfg(target_os = "windows")]
use std::ffi::OsStr;
#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Printing::{
    ClosePrinter, EndDocPrinter, EndPagePrinter, OpenPrinterW, StartDocPrinterW, StartPagePrinter,
    WritePrinter, DOC_INFO_1W,
};
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HANDLE;
#[cfg(target_os = "windows")]
use windows::core::{PWSTR, PCWSTR};

pub struct WindowsPrinterEngine;

impl WindowsPrinterEngine {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsPrinterEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "windows")]
impl PrinterEngine for WindowsPrinterEngine {
    fn print(&self, printer_name: &str, data: &[u8]) -> Result<(), InfrastructureError> {
        tracing::info!(
            target = "sapo_printer::printer_engine",
            printer = printer_name,
            data_size = data.len(),
            "WindowsPrinterEngine: starting print"
        );

        // Special handling for "Microsoft Print to PDF"
        // This driver doesn't support raw PDF data via WritePrinter()
        // Instead, save directly to file
        if printer_name.contains("Microsoft Print to PDF") || printer_name.contains("Print to PDF") {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let output_path = format!(
                "C:\\Users\\{}\\Documents\\SAPO_Print_{}.pdf",
                std::env::var("USERNAME").unwrap_or_else(|_| "User".to_string()),
                timestamp
            );

            tracing::info!(
                target = "sapo_printer::printer_engine",
                output_path = output_path,
                "Saving PDF directly to file (Microsoft Print to PDF workaround)"
            );

            std::fs::write(&output_path, data).map_err(|e| {
                InfrastructureError::PrinterError {
                    reason: format!("Failed to write PDF file: {}", e),
                }
            })?;

            tracing::info!(
                target = "sapo_printer::printer_engine",
                output_path = output_path,
                bytes_written = data.len(),
                "PDF saved successfully"
            );

            return Ok(());
        }

        // For real printers, use Windows printer API
        unsafe {
            // Convert printer name to wide string
            let printer_name_wide: Vec<u16> = OsStr::new(printer_name)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let mut h_printer = HANDLE::default();

            // Open printer
            tracing::debug!(
                target = "sapo_printer::printer_engine",
                printer = printer_name,
                "Opening printer handle"
            );

            let result = OpenPrinterW(
                PCWSTR(printer_name_wide.as_ptr()),
                &mut h_printer,
                None,
            );

            if result.is_err() {
                return Err(InfrastructureError::PrinterError {
                    reason: format!("Failed to open printer '{}': {:?}", printer_name, result.err()),
                });
            }

            // Start document
            let doc_name_wide: Vec<u16> = OsStr::new("SAPO Print Job")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            // Generate output file path for "Microsoft Print to PDF"
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let output_path = format!(
                "C:\\Users\\{}\\Documents\\SAPO_Print_{}.pdf",
                std::env::var("USERNAME").unwrap_or_else(|_| "User".to_string()),
                timestamp
            );
            let output_path_wide: Vec<u16> = OsStr::new(&output_path)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            // Set datatype to RAW for direct PDF data
            let datatype_wide: Vec<u16> = OsStr::new("RAW")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let doc_info = DOC_INFO_1W {
                pDocName: PWSTR(doc_name_wide.as_ptr() as *mut u16),
                pOutputFile: PWSTR(output_path_wide.as_ptr() as *mut u16),
                pDatatype: PWSTR(datatype_wide.as_ptr() as *mut u16),
            };

            tracing::debug!(
                target = "sapo_printer::printer_engine",
                output_file = output_path,
                "Starting document with output path"
            );

            let job_id = StartDocPrinterW(h_printer, 1, &doc_info);
            if job_id == 0 {
                ClosePrinter(h_printer);
                return Err(InfrastructureError::PrinterError {
                    reason: "Failed to start document".to_string(),
                });
            }

            tracing::info!(
                target = "sapo_printer::printer_engine",
                job_id = job_id,
                "Document started, job ID assigned"
            );

            // Start page
            let page_result = StartPagePrinter(h_printer);
            if !page_result.as_bool() {
                EndDocPrinter(h_printer);
                ClosePrinter(h_printer);
                return Err(InfrastructureError::PrinterError {
                    reason: "Failed to start page".to_string(),
                });
            }

            tracing::debug!(
                target = "sapo_printer::printer_engine",
                "Page started"
            );

            // Write data
            let mut bytes_written = 0u32;
            let write_result = WritePrinter(
                h_printer,
                data.as_ptr() as *const _,
                data.len() as u32,
                &mut bytes_written,
            );

            if !write_result.as_bool() {
                EndPagePrinter(h_printer);
                EndDocPrinter(h_printer);
                ClosePrinter(h_printer);
                return Err(InfrastructureError::PrinterError {
                    reason: "Failed to write data to printer".to_string(),
                });
            }

            tracing::info!(
                target = "sapo_printer::printer_engine",
                bytes_written = bytes_written,
                bytes_total = data.len(),
                "Data written to printer"
            );

            // End page
            let end_page_result = EndPagePrinter(h_printer);
            if !end_page_result.as_bool() {
                tracing::warn!(
                    target = "sapo_printer::printer_engine",
                    "Failed to end page (non-fatal)"
                );
            }

            // End document
            let end_doc_result = EndDocPrinter(h_printer);
            if !end_doc_result.as_bool() {
                ClosePrinter(h_printer);
                return Err(InfrastructureError::PrinterError {
                    reason: "Failed to end document".to_string(),
                });
            }

            tracing::debug!(
                target = "sapo_printer::printer_engine",
                "Document ended"
            );

            // Close printer
            let _ = ClosePrinter(h_printer);

            tracing::info!(
                target = "sapo_printer::printer_engine",
                printer = printer_name,
                job_id = job_id,
                "WindowsPrinterEngine: print completed successfully"
            );

            Ok(())
        }
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    #[test]
    fn test_print_stub_returns_ok() {
        let engine = WindowsPrinterEngine::new();
        assert!(engine.print("any_printer", &[]).is_ok());
    }
}
