use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::application::dto::PrinterDto;
use crate::application::ports::{PrinterAvailability, PrinterManager};
use crate::shared::errors::InfrastructureError;

const LIST_CACHE_TTL: Duration = Duration::from_secs(5);

pub struct SystemPrinterManager {
    cache: Mutex<Option<(Instant, Vec<PrinterDto>)>>,
}

impl SystemPrinterManager {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(None),
        }
    }

    fn list_cached(&self) -> Result<Vec<PrinterDto>, InfrastructureError> {
        {
            let cache = self.cache.lock().unwrap_or_else(|p| p.into_inner());
            if let Some((cached_at, printers)) = cache.as_ref() {
                if cached_at.elapsed() < LIST_CACHE_TTL {
                    return Ok(printers.clone());
                }
            }
        }

        let printers = list_os_printers()?;

        let mut cache = self.cache.lock().unwrap_or_else(|p| p.into_inner());
        *cache = Some((Instant::now(), printers.clone()));
        Ok(printers)
    }
}

impl Default for SystemPrinterManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PrinterManager for SystemPrinterManager {
    fn list(&self) -> Result<Vec<PrinterDto>, InfrastructureError> {
        self.list_cached()
    }

    fn availability(&self, printer_id: &str) -> Result<PrinterAvailability, InfrastructureError> {
        let printers = self.list_cached()?;
        let found = printers.into_iter().find(|p| p.id == printer_id);

        match found {
            None => Ok(PrinterAvailability::Unknown),
            Some(p) if p.status.eq_ignore_ascii_case("Online") => Ok(PrinterAvailability::Online),
            Some(_) => Ok(PrinterAvailability::Offline),
        }
    }
}

#[cfg(target_os = "windows")]
fn win32_status_to_str(status: u32) -> &'static str {
    // Win32 PRINTER_STATUS_* bit flags from winspool.h
    const PRINTER_STATUS_OFFLINE: u32 = 0x00000080;
    const PRINTER_STATUS_NOT_AVAILABLE: u32 = 0x00001000;
    const PRINTER_STATUS_ERROR: u32 = 0x00000002;

    if status & (PRINTER_STATUS_OFFLINE | PRINTER_STATUS_NOT_AVAILABLE) != 0 {
        "Offline"
    } else if status & PRINTER_STATUS_ERROR != 0 {
        "Error"
    } else {
        // 0 = Normal/Ready; all other non-fatal flags (Busy, Printing, WarmingUp…) = still usable
        "Online"
    }
}

#[cfg(target_os = "windows")]
fn get_default_printer_name() -> String {
    use windows::Win32::Graphics::Printing::GetDefaultPrinterW;
    use windows::core::PWSTR;

    unsafe {
        let mut size: u32 = 0;
        // First call returns false and sets `size` to the required buffer length (chars).
        GetDefaultPrinterW(PWSTR::null(), &mut size);
        if size == 0 {
            return String::new();
        }
        let mut buf = vec![0u16; size as usize];
        if GetDefaultPrinterW(PWSTR(buf.as_mut_ptr()), &mut size).as_bool() {
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            String::from_utf16_lossy(&buf[..len])
        } else {
            String::new()
        }
    }
}

#[cfg(target_os = "windows")]
fn list_os_printers() -> Result<Vec<PrinterDto>, InfrastructureError> {
    use windows::Win32::Graphics::Printing::{EnumPrintersW, PRINTER_INFO_2W};
    use windows::core::PCWSTR;

    // PRINTER_ENUM_LOCAL(2) | PRINTER_ENUM_CONNECTIONS(4)
    const FLAGS: u32 = 2 | 4;

    let default_printer = get_default_printer_name();
    let mut bytes_needed: u32 = 0;
    let mut count: u32 = 0;

    // First call — always returns false with ERROR_INSUFFICIENT_BUFFER; used only to get size.
    unsafe {
        let _ = EnumPrintersW(
            FLAGS,
            PCWSTR::null(),
            2,
            None,
            &mut bytes_needed,
            &mut count,
        );
    }

    if bytes_needed == 0 {
        return Ok(vec![]);
    }

    let mut buffer = vec![0u8; bytes_needed as usize];

    unsafe {
        EnumPrintersW(
            FLAGS,
            PCWSTR::null(),
            2,
            Some(&mut buffer),
            &mut bytes_needed,
            &mut count,
        )
        .map_err(|e| InfrastructureError::PrinterError {
            reason: format!("EnumPrintersW failed: {}", e),
        })?;
    }

    let mut printers = Vec::new();

    for i in 0..count as usize {
        // SAFETY: EnumPrintersW packed `count` PRINTER_INFO_2W structs at the start of `buffer`;
        // string pointers inside each struct point into the tail of the same buffer.
        let info = unsafe {
            &*(buffer.as_ptr().add(i * size_of::<PRINTER_INFO_2W>()) as *const PRINTER_INFO_2W)
        };

        if info.pPrinterName.is_null() {
            continue;
        }

        let name = unsafe { info.pPrinterName.to_string().unwrap_or_default() };
        if name.is_empty() {
            continue;
        }

        let is_default = name == default_printer;
        printers.push(PrinterDto {
            id: name.clone(),
            name,
            status: win32_status_to_str(info.Status).to_string(),
            printer_type: "Local".to_string(),
            is_default,
        });
    }

    Ok(printers)
}

#[cfg(not(target_os = "windows"))]
fn list_os_printers() -> Result<Vec<PrinterDto>, InfrastructureError> {
    use std::process::Command;

    let default_printer = Command::new("lpstat")
        .arg("-d")
        .output()
        .ok()
        .and_then(|o| {
            let text = String::from_utf8_lossy(&o.stdout).to_string();
            text.split(':').nth(1).map(|s| s.trim().to_string())
        })
        .unwrap_or_default();

    let output = Command::new("lpstat").arg("-p").output().map_err(|e| {
        InfrastructureError::PrinterError {
            reason: format!("Failed to execute lpstat: {}", e),
        }
    })?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut printers = Vec::new();

    for line in text.lines() {
        if line.starts_with("printer ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let name = parts[1].to_string();
                let status = if line.contains("idle") || line.contains("printing") {
                    "Online"
                } else if line.contains("disabled") {
                    "Offline"
                } else {
                    "Unknown"
                };
                let is_default = name == default_printer;

                printers.push(PrinterDto {
                    id: name.clone(),
                    name,
                    status: status.to_string(),
                    printer_type: "Local".to_string(),
                    is_default,
                });
            }
        }
    }
    Ok(printers)
}
