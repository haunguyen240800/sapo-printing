#[cfg(target_os = "windows")]
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::HANDLE,
        Graphics::Printing::{
            ClosePrinter, EnumPrintersW, GetPrinterW, OpenPrinterW, PRINTER_ATTRIBUTE_NETWORK,
            PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL, PRINTER_INFO_2W, PRINTER_STATUS_ERROR,
            PRINTER_STATUS_NO_TONER, PRINTER_STATUS_OFFLINE, PRINTER_STATUS_OUTPUT_BIN_FULL,
            PRINTER_STATUS_PAPER_JAM, PRINTER_STATUS_PAPER_OUT,
        },
    },
};

use super::super::printer_manager::PrinterManager;
use crate::domain::printer::{Printer, PrinterName, PrinterStatus, PrinterType};

pub struct Win32PrinterManager;

impl Win32PrinterManager {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Win32PrinterManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Maps a raw Win32 printer status bitmask to [`PrinterStatus`].
/// Extracted as a standalone function so unit tests can call it without Win32.
fn map_win32_status(status: u32) -> PrinterStatus {
    #[cfg(target_os = "windows")]
    {
        const ERROR_FLAGS: u32 = PRINTER_STATUS_ERROR
            | PRINTER_STATUS_PAPER_JAM
            | PRINTER_STATUS_PAPER_OUT
            | PRINTER_STATUS_OUTPUT_BIN_FULL
            | PRINTER_STATUS_NO_TONER;

        if status & PRINTER_STATUS_OFFLINE != 0 {
            PrinterStatus::Offline
        } else if status & ERROR_FLAGS != 0 {
            PrinterStatus::Error
        } else {
            PrinterStatus::Online
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = status;
        PrinterStatus::Online
    }
}

/// Reads a `PWSTR` to `String`. Private helper to contain unsafe.
#[cfg(target_os = "windows")]
fn read_pwstr(s: windows::core::PWSTR) -> String {
    if s.is_null() {
        return String::new();
    }
    unsafe { s.to_string().unwrap_or_default() }
}

/// Calls `EnumPrintersW` level 2.
/// Returns `(buffer, count)` — caller must keep buffer alive while reading structs.
#[cfg(target_os = "windows")]
fn enum_printers_raw() -> (Vec<u8>, usize) {
    unsafe {
        let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
        let mut bytes_needed: u32 = 0;
        let mut count_returned: u32 = 0;

        let _ = EnumPrintersW(
            flags,
            PCWSTR::null(),
            2,
            None,
            &mut bytes_needed,
            &mut count_returned,
        );

        if bytes_needed == 0 {
            return (vec![], 0);
        }

        let mut buf = vec![0u8; bytes_needed as usize];
        if EnumPrintersW(
            flags,
            PCWSTR::null(),
            2,
            Some(buf.as_mut_slice()),
            &mut bytes_needed,
            &mut count_returned,
        )
        .is_err()
        {
            return (vec![], 0);
        }

        (buf, count_returned as usize)
    }
}

/// Queries printer status via `OpenPrinterW` + `GetPrinterW` level 2.
/// `ClosePrinter` is always called when the handle was successfully opened.
#[cfg(target_os = "windows")]
fn query_printer_status(name: &str) -> PrinterStatus {
    unsafe {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut handle = HANDLE::default();

        if OpenPrinterW(PCWSTR(wide.as_ptr()), &mut handle, None).is_err() {
            return PrinterStatus::Offline;
        }

        let mut needed: u32 = 0;
        let _ = GetPrinterW(handle, 2, None, &mut needed);

        if needed == 0 {
            ClosePrinter(handle).ok();
            return PrinterStatus::Offline;
        }

        let mut buf = vec![0u8; needed as usize];
        let ok = GetPrinterW(handle, 2, Some(buf.as_mut_slice()), &mut needed);
        ClosePrinter(handle).ok(); // always close

        if ok.is_err() {
            return PrinterStatus::Offline;
        }

        let info = &*(buf.as_ptr() as *const PRINTER_INFO_2W);
        map_win32_status(info.Status)
    }
}

/// Checks if a printer driver name suggests native PDF support.
///
/// Heuristic: matches known PDF-capable driver keywords.
/// Conservative — false negatives are acceptable (fallback to PDFium),
/// false positives are BAD (print will fail).
fn driver_name_suggests_pdf(driver_name: &str) -> bool {
    let upper = driver_name.to_uppercase();
    upper.contains("PDF")
        || upper.contains("POSTSCRIPT")
        || upper.contains(" PS ")
        || upper.ends_with(" PS")
        || upper.starts_with("PS ")
}

#[cfg(target_os = "windows")]
fn detect_direct_pdf_support(printer_name: &str) -> bool {
    unsafe {
        let wide: Vec<u16> = printer_name
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut handle = HANDLE::default();

        if OpenPrinterW(PCWSTR(wide.as_ptr()), &mut handle, None).is_err() {
            tracing::warn!(
                "supports_direct_pdf: OpenPrinterW failed for '{}', returning false",
                printer_name
            );
            return false;
        }

        let mut needed: u32 = 0;
        let _ = GetPrinterW(handle, 2, None, &mut needed);

        if needed == 0 {
            ClosePrinter(handle).ok();
            tracing::warn!(
                "supports_direct_pdf: GetPrinterW returned 0 bytes for '{}', returning false",
                printer_name
            );
            return false;
        }

        let mut buf = vec![0u8; needed as usize];
        let ok = GetPrinterW(handle, 2, Some(buf.as_mut_slice()), &mut needed);
        ClosePrinter(handle).ok();

        if ok.is_err() {
            tracing::warn!(
                "supports_direct_pdf: GetPrinterW failed for '{}', returning false",
                printer_name
            );
            return false;
        }

        if (needed as usize) < std::mem::size_of::<PRINTER_INFO_2W>() {
            tracing::warn!(
                "supports_direct_pdf: buffer too small ({} bytes) for PRINTER_INFO_2W for '{}', returning false",
                needed,
                printer_name
            );
            return false;
        }

        let info = &*(buf.as_ptr() as *const PRINTER_INFO_2W);

        let driver_name = read_pwstr(info.pDriverName);

        // PRINTER_ATTRIBUTE_RAW_ONLY alone is not sufficient — PCL/ESC/P printers
        // can have this flag but cannot interpret raw PDF bytes.
        // Combine with driver name heuristic: RAW_ONLY confirms raw data acceptance,
        // driver name confirms the raw format is PDF.
        // Driver name alone is also sufficient (some PDF drivers don't set RAW_ONLY).
        if driver_name_suggests_pdf(&driver_name) {
            return true;
        }

        false
    }
}

impl PrinterManager for Win32PrinterManager {
    fn discover_printers(&self) -> Vec<Printer> {
        #[cfg(target_os = "windows")]
        {
            let (buf, count) = enum_printers_raw();
            if count == 0 {
                return vec![];
            }
            // SAFETY: buf is kept alive for the duration of this block.
            // PRINTER_INFO_2W.pPrinterName points into buf; read_pwstr copies before buf drops.
            unsafe {
                let ptr = buf.as_ptr() as *const PRINTER_INFO_2W;
                std::slice::from_raw_parts(ptr, count)
                    .iter()
                    .map(|info| {
                        let name = read_pwstr(info.pPrinterName);
                        let printer_type = if info.Attributes & PRINTER_ATTRIBUTE_NETWORK != 0 {
                            PrinterType::Network
                        } else {
                            PrinterType::Local
                        };
                        Printer::new(PrinterName::new(name), printer_type)
                    })
                    .collect()
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            vec![]
        }
    }

    fn get_status(&self, name: &str) -> PrinterStatus {
        #[cfg(target_os = "windows")]
        {
            query_printer_status(name)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = name;
            PrinterStatus::Offline
        }
    }

    fn supports_direct_pdf(&self, printer_name: &str) -> bool {
        #[cfg(target_os = "windows")]
        {
            detect_direct_pdf_support(printer_name)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = printer_name;
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::printer::PrinterStatus;

    #[test]
    fn test_status_mapping_zero_is_online() {
        assert_eq!(map_win32_status(0), PrinterStatus::Online);
    }

    #[test]
    fn test_status_mapping_offline_flag() {
        // PRINTER_STATUS_OFFLINE = 0x80 = 128
        assert_eq!(map_win32_status(0x00000080), PrinterStatus::Offline);
    }

    #[test]
    fn test_status_mapping_error_flag() {
        // PRINTER_STATUS_ERROR = 0x2
        assert_eq!(map_win32_status(0x00000002), PrinterStatus::Error);
    }

    #[test]
    fn test_discover_returns_vec() {
        // Must not panic — may return empty Vec in CI
        let manager = Win32PrinterManager::new();
        let _printers = manager.discover_printers();
    }

    #[test]
    fn test_driver_name_pdf_keywords() {
        assert!(driver_name_suggests_pdf("Microsoft Print to PDF"));
        assert!(driver_name_suggests_pdf("Adobe PDF"));
        assert!(driver_name_suggests_pdf("HP LaserJet PS"));
        assert!(driver_name_suggests_pdf("HP PostScript"));
    }

    #[test]
    fn test_driver_name_non_pdf() {
        assert!(!driver_name_suggests_pdf("HP LaserJet 4000"));
        assert!(!driver_name_suggests_pdf("Epson L3150 Series"));
        assert!(!driver_name_suggests_pdf("Brother HL-L2350DW"));
        assert!(
            !driver_name_suggests_pdf("Canon PCL6"),
            "PCL is not PDF — must return false to avoid false positives"
        );
    }

    #[test]
    fn test_supports_direct_pdf_nonexistent_printer() {
        let manager = Win32PrinterManager::new();
        assert!(
            !manager.supports_direct_pdf("NonExistentPrinter_XYZ_99999"),
            "Non-existent printer must return false"
        );
    }
}
