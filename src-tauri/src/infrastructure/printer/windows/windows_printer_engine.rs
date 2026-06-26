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
    fn print(
        &self,
        printer_name: &str,
        data: &[u8],
        output_path: Option<&str>,
    ) -> Result<(), InfrastructureError> {
        tracing::info!(
            target = "sapo_printer::printer_engine",
            printer = printer_name,
            data_size = data.len(),
            "WindowsPrinterEngine: starting print"
        );

        if data.is_empty() {
            return Err(InfrastructureError::ValidationError(
                "Print data is empty".to_string(),
            ));
        }

        // Detect if data is a raw PDF
        let is_pdf = data.starts_with(b"%PDF");

        if is_pdf {
            // Special handling for "Microsoft Print to PDF"
            // This driver doesn't support raw PDF data via WritePrinter()
            // Instead, save directly to file
            if printer_name.contains("Microsoft Print to PDF") || printer_name.contains("Print to PDF") {
                // Use provided output_path or generate auto path
                let output_file_path = match output_path {
                    Some(path) => path.to_string(),
                    None => {
                        let timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        format!(
                            "C:\\Users\\{}\\Documents\\SAPO_Print_{}.pdf",
                            std::env::var("USERNAME").unwrap_or_else(|_| "User".to_string()),
                            timestamp
                        )
                    }
                };

                tracing::info!(
                    target = "sapo_printer::printer_engine",
                    output_path = output_file_path,
                    "Saving PDF directly to file (Microsoft Print to PDF workaround)"
                );

                std::fs::write(&output_file_path, data).map_err(|e| {
                    InfrastructureError::PrinterError {
                        reason: format!("Failed to write PDF file: {}", e),
                    }
                })?;

                tracing::info!(
                    target = "sapo_printer::printer_engine",
                    output_path = output_file_path,
                    bytes_written = data.len(),
                    "PDF saved successfully"
                );

                return Ok(());
            }

            // Otherwise, send RAW data via Spooler
            self.print_raw(printer_name, data, output_path)
        } else {
            // Rendered bitmap custom format
            self.print_gdi_bitmap(printer_name, data)
        }
    }
}

impl WindowsPrinterEngine {
    fn print_raw(
        &self,
        printer_name: &str,
        data: &[u8],
        output_path: Option<&str>,
    ) -> Result<(), InfrastructureError> {
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
                "Opening printer handle (RAW)"
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

            // Set datatype to RAW for direct PDF data
            let datatype_wide: Vec<u16> = OsStr::new("RAW")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            // Handle optional PDF output path
            let output_path_wide = output_path.map(|p| {
                OsStr::new(p).encode_wide().chain(std::iter::once(0)).collect::<Vec<u16>>()
            });

            let doc_info = DOC_INFO_1W {
                pDocName: PWSTR(doc_name_wide.as_ptr() as *mut u16),
                pOutputFile: match &output_path_wide {
                    Some(wide) => PWSTR(wide.as_ptr() as *mut u16),
                    None => PWSTR::null(),
                },
                pDatatype: PWSTR(datatype_wide.as_ptr() as *mut u16),
            };

            tracing::debug!(
                target = "sapo_printer::printer_engine",
                output_file = ?output_path,
                "Starting document (RAW)"
            );

            let job_id = StartDocPrinterW(h_printer, 1, &doc_info);
            if job_id == 0 {
                let _ = ClosePrinter(h_printer);
                return Err(InfrastructureError::PrinterError {
                    reason: "Failed to start document".to_string(),
                });
            }

            tracing::info!(
                target = "sapo_printer::printer_engine",
                job_id = job_id,
                "Document started (RAW), job ID assigned"
            );

            // Start page
            let page_result = StartPagePrinter(h_printer);
            if !page_result.as_bool() {
                EndDocPrinter(h_printer);
                let _ = ClosePrinter(h_printer);
                return Err(InfrastructureError::PrinterError {
                    reason: "Failed to start page".to_string(),
                });
            }

            tracing::debug!(
                target = "sapo_printer::printer_engine",
                "Page started (RAW)"
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
                let _ = EndPagePrinter(h_printer);
                let _ = EndDocPrinter(h_printer);
                let _ = ClosePrinter(h_printer);
                return Err(InfrastructureError::PrinterError {
                    reason: "Failed to write data to printer".to_string(),
                });
            }

            tracing::info!(
                target = "sapo_printer::printer_engine",
                bytes_written = bytes_written,
                bytes_total = data.len(),
                "Data written to printer (RAW)"
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
                let _ = ClosePrinter(h_printer);
                return Err(InfrastructureError::PrinterError {
                    reason: "Failed to end document".to_string(),
                });
            }

            tracing::debug!(
                target = "sapo_printer::printer_engine",
                "Document ended (RAW)"
            );

            // Close printer
            let _ = ClosePrinter(h_printer);

            tracing::info!(
                target = "sapo_printer::printer_engine",
                printer = printer_name,
                job_id = job_id,
                "WindowsPrinterEngine: RAW print completed successfully"
            );

            Ok(())
        }
    }

    #[cfg(target_os = "windows")]
    fn print_gdi_bitmap(
        &self,
        printer_name: &str,
        data: &[u8],
    ) -> Result<(), InfrastructureError> {
        use windows::Win32::Graphics::Gdi::{
            CreateDCW, DeleteDC, StretchDIBits,
            BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, GetDeviceCaps,
            PHYSICALHEIGHT, PHYSICALOFFSETX, PHYSICALOFFSETY, PHYSICALWIDTH, SRCCOPY,
        };
        use windows::Win32::Storage::Xps::{
            StartDocW, StartPage, EndDoc, EndPage, DOCINFOW
        };

        if data.len() < 4 {
            return Err(InfrastructureError::ValidationError(
                "Rendered data is too small to contain page count".to_string(),
            ));
        }

        unsafe {
            let printer_name_wide: Vec<u16> = std::ffi::OsStr::new(printer_name)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let hdc = CreateDCW(
                PCWSTR::null(),
                PCWSTR(printer_name_wide.as_ptr()),
                PCWSTR::null(),
                None,
            );

            if hdc.is_invalid() {
                return Err(InfrastructureError::PrinterError {
                    reason: format!("Failed to create GDI device context for '{}'", printer_name),
                });
            }

            let doc_name_wide: Vec<u16> = std::ffi::OsStr::new("SAPO Print Job (Bitmap)")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let doc_info = DOCINFOW {
                cbSize: std::mem::size_of::<DOCINFOW>() as i32,
                lpszDocName: PCWSTR(doc_name_wide.as_ptr()),
                lpszOutput: PCWSTR::null(),
                lpszDatatype: PCWSTR::null(),
                fwType: 0,
            };

            let job_id = StartDocW(hdc, &doc_info);
            if job_id <= 0 {
                DeleteDC(hdc);
                return Err(InfrastructureError::PrinterError {
                    reason: "Failed to start GDI document".to_string(),
                });
            }

            tracing::info!(
                target = "sapo_printer::printer_engine",
                job_id = job_id,
                printer = printer_name,
                "Document started (GDI), job ID assigned"
            );

            let mut offset = 0;
            let page_count = u32::from_le_bytes(data[0..4].try_into().unwrap());
            offset += 4;

            let phys_w = GetDeviceCaps(hdc, PHYSICALWIDTH);
            let phys_h = GetDeviceCaps(hdc, PHYSICALHEIGHT);
            let off_x = GetDeviceCaps(hdc, PHYSICALOFFSETX);
            let off_y = GetDeviceCaps(hdc, PHYSICALOFFSETY);

            for page_idx in 0..page_count {
                if offset + 8 > data.len() {
                    break;
                }

                let w = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
                offset += 4;
                let h = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
                offset += 4;

                let bytes = (w * h * 4) as usize; // Argb
                if offset + bytes > data.len() {
                    break;
                }

                let pixels = &data[offset..offset + bytes];
                offset += bytes;

                tracing::debug!(
                    target = "sapo_printer::printer_engine",
                    page = page_idx + 1,
                    w = w,
                    h = h,
                    "Printing GDI page"
                );

                if StartPage(hdc) <= 0 {
                    break;
                }

                let bmi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: w as i32,
                        biHeight: -(h as i32), // top-down DIB
                        biPlanes: 1,
                        biBitCount: 24, // BGR 24-bit with 4-byte padding
                        biCompression: BI_RGB.0,
                        biSizeImage: 0,
                        biXPelsPerMeter: 0,
                        biYPelsPerMeter: 0,
                        biClrUsed: 0,
                        biClrImportant: 0,
                    },
                    bmiColors: [windows::Win32::Graphics::Gdi::RGBQUAD::default(); 1],
                };

                let scan_lines_copied = StretchDIBits(
                    hdc,
                    -off_x,
                    -off_y,
                    phys_w,
                    phys_h,
                    0,
                    0,
                    w as i32,
                    h as i32,
                    Some(pixels.as_ptr() as *const core::ffi::c_void),
                    &bmi,
                    DIB_RGB_COLORS,
                    SRCCOPY,
                );

                if scan_lines_copied == 0 {
                    tracing::warn!(
                        target = "sapo_printer::printer_engine",
                        "StretchDIBits copied 0 scan lines (possible failure)"
                    );
                }

                EndPage(hdc);
            }

            EndDoc(hdc);
            DeleteDC(hdc);

            tracing::info!(
                target = "sapo_printer::printer_engine",
                printer = printer_name,
                job_id = job_id,
                "WindowsPrinterEngine: GDI print completed successfully"
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
        assert!(engine.print("any_printer", &[], None).is_ok());
    }
}
