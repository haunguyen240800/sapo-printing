#[cfg(not(target_os = "windows"))]
use super::super::printer_manager::PrinterManager;
#[cfg(not(target_os = "windows"))]
use crate::domain::printer::{Printer, PrinterName, PrinterStatus, PrinterType};

#[cfg(not(target_os = "windows"))]
pub struct CupsPrinterManager;

#[cfg(not(target_os = "windows"))]
impl CupsPrinterManager {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(target_os = "windows"))]
impl Default for CupsPrinterManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_os = "windows"))]
impl PrinterManager for CupsPrinterManager {
    fn discover_printers(&self) -> Vec<Printer> {
        // Tier 1: CUPS API
        let printers = cups_api_discover();
        if !printers.is_empty() {
            return printers;
        }
        // Tier 2: lpstat
        let printers = lpstat_discover();
        if !printers.is_empty() {
            return printers;
        }
        // Tier 3: printers.conf
        conf_discover()
    }

    fn get_status(&self, name: &str) -> PrinterStatus {
        get_status(name)
    }

    fn supports_direct_pdf(&self, printer_name: &str) -> bool {
        detect_cups_direct_pdf(printer_name)
    }
}

// ── Tier 1: CUPS API ────────────────────────────────────────────────

#[cfg(not(target_os = "windows"))]
fn cups_api_discover() -> Vec<Printer> {
    use cups_sys::{cupsFreeDests, cupsGetDests, cups_dest_t};
    use std::ffi::CStr;

    unsafe {
        let mut dests: *mut cups_dest_t = std::mem::zeroed();
        let count = cupsGetDests(&mut dests as *mut _);
        if count <= 0 || dests.is_null() {
            return vec![];
        }

        let destinations = std::slice::from_raw_parts(dests, count as usize);
        let result = destinations
            .iter()
            .map(|dest| {
                let name = if dest.name.is_null() {
                    String::new()
                } else {
                    CStr::from_ptr(dest.name).to_string_lossy().into_owned()
                };
                let printer_type = detect_printer_type_from_dest(dest);
                let mut printer = Printer::new(PrinterName::new(name), printer_type);

                // Check printer-state option: "3" = idle (Online)
                if get_printer_state(dest) == 3 {
                    let _ = printer.connect();
                }

                printer
            })
            .collect();

        cupsFreeDests(count, dests);
        result
    }
}

#[cfg(not(target_os = "windows"))]
fn detect_printer_type_from_dest(dest: &cups_sys::cups_dest_t) -> PrinterType {
    use cups_sys::cups_option_t;
    use std::ffi::CStr;

    if dest.num_options <= 0 || dest.options.is_null() {
        return PrinterType::Local;
    }

    let opts = unsafe {
        std::slice::from_raw_parts(
            dest.options as *const cups_option_t,
            dest.num_options as usize,
        )
    };

    for opt in opts {
        if !opt.name.is_null() {
            let key = unsafe { CStr::from_ptr(opt.name).to_string_lossy() };
            if key == "device-uri" && !opt.value.is_null() {
                let val = unsafe { CStr::from_ptr(opt.value).to_string_lossy() };
                if val.starts_with("ipp://")
                    || val.starts_with("ipps://")
                    || val.starts_with("socket://")
                {
                    return PrinterType::Network;
                }
            }
        }
    }
    PrinterType::Local
}

#[cfg(not(target_os = "windows"))]
fn get_printer_state(dest: &cups_sys::cups_dest_t) -> i32 {
    use cups_sys::cups_option_t;
    use std::ffi::CStr;

    if dest.num_options <= 0 || dest.options.is_null() {
        return 3; // default to idle (Online)
    }

    let opts = unsafe {
        std::slice::from_raw_parts(
            dest.options as *const cups_option_t,
            dest.num_options as usize,
        )
    };

    for opt in opts {
        if !opt.name.is_null() {
            let key = unsafe { CStr::from_ptr(opt.name).to_string_lossy() };
            if key == "printer-state" && !opt.value.is_null() {
                let val = unsafe { CStr::from_ptr(opt.value).to_string_lossy() };
                if let Ok(state) = val.parse::<i32>() {
                    return state;
                }
            }
        }
    }
    3 // default to idle
}

// ── Tier 2: lpstat ──────────────────────────────────────────────────

#[cfg(not(target_os = "windows"))]
fn lpstat_discover() -> Vec<Printer> {
    let output = std::process::Command::new("lpstat")
        .args(["-p", "-d"])
        .output();

    let output = match output {
        Ok(o) => o,
        Err(_) => return vec![],
    };

    if !output.status.success() || output.stdout.is_empty() {
        return vec![];
    }

    let text = String::from_utf8_lossy(&output.stdout);
    parse_lpstat_output(&text)
}

#[cfg(not(target_os = "windows"))]
fn parse_lpstat_output(text: &str) -> Vec<Printer> {
    text.lines()
        .filter_map(|line| {
            // Expected format: "printer <name> is idle..." or "printer <name> disabled..."
            let line = line.strip_prefix("printer ")?;

            // Split on " is " or " disabled" to extract name
            let (name, rest) = if let Some(pos) = line.find(" is ") {
                (&line[..pos], &line[pos + 4..])
            } else if let Some(pos) = line.find(" disabled") {
                (&line[..pos], "disabled")
            } else {
                return None;
            };

            let mut printer = Printer::new(PrinterName::new(name.to_string()), PrinterType::Local);

            // "disabled" → Offline, otherwise try to set Online
            if rest.starts_with("disabled") || rest == "disabled" {
                // Printer starts as Offline, no action needed
            } else {
                // Try to connect (set Online)
                let _ = printer.connect();
            }

            Some(printer)
        })
        .collect()
}

// ── Tier 3: printers.conf ───────────────────────────────────────────

#[cfg(not(target_os = "windows"))]
fn conf_discover() -> Vec<Printer> {
    let path = std::path::Path::new("/etc/cups/printers.conf");
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    parse_printers_conf(&content)
}

#[cfg(not(target_os = "windows"))]
fn parse_printers_conf(content: &str) -> Vec<Printer> {
    let mut printers = vec![];
    let mut current_name: Option<String> = None;
    let mut current_stopped = false;

    for line in content.lines() {
        let line = line.trim();
        if let Some(name) = line
            .strip_prefix("<Printer ")
            .and_then(|s| s.strip_suffix('>'))
        {
            current_name = Some(name.to_string());
            current_stopped = false;
        } else if line == "</Printer>" {
            if let Some(name) = current_name.take() {
                let mut printer = Printer::new(PrinterName::new(name), PrinterType::Local);
                // If not stopped, set Online
                if !current_stopped {
                    let _ = printer.connect();
                }
                printers.push(printer);
            }
        } else if line.starts_with("State ") {
            current_stopped = line == "State Stopped";
        }
    }
    printers
}

// ── get_status() ────────────────────────────────────────────────────

#[cfg(not(target_os = "windows"))]
fn get_status(name: &str) -> PrinterStatus {
    // Tier 1: CUPS API
    if let Some(status) = cups_api_get_status(name) {
        return status;
    }
    // Tier 2: lpstat
    if let Some(status) = lpstat_get_status(name) {
        return status;
    }
    // Tier 3: printers.conf — if not found anywhere → Offline
    PrinterStatus::Offline
}

#[cfg(not(target_os = "windows"))]
fn cups_api_get_status(name: &str) -> Option<PrinterStatus> {
    use cups_sys::{cupsFreeDests, cupsGetDests, cups_dest_t};
    use std::ffi::CStr;

    unsafe {
        let mut dests: *mut cups_dest_t = std::mem::zeroed();
        let count = cupsGetDests(&mut dests as *mut _);

        // Must free dests even if count <= 0
        if count <= 0 {
            if !dests.is_null() {
                cupsFreeDests(count, dests);
            }
            return None;
        }

        if dests.is_null() {
            return None;
        }

        let destinations = std::slice::from_raw_parts(dests, count as usize);
        let mut result = None;

        for dest in destinations {
            let dest_name = if dest.name.is_null() {
                String::new()
            } else {
                CStr::from_ptr(dest.name).to_string_lossy().into_owned()
            };

            if dest_name == name {
                let state = get_printer_state(dest);
                result = Some(match state {
                    3 => PrinterStatus::Online, // IPP_PRINTER_IDLE
                    4 => PrinterStatus::Online, // IPP_PRINTER_PROCESSING — changed per review
                    5 => PrinterStatus::Error,  // IPP_PRINTER_STOPPED
                    _ => PrinterStatus::Online,
                });
                break;
            }
        }

        cupsFreeDests(count, dests);

        if result.is_none() {
            // Printer not found in CUPS → treat as Offline
            result = Some(PrinterStatus::Offline);
        }

        result
    }
}

#[cfg(not(target_os = "windows"))]
fn lpstat_get_status(name: &str) -> Option<PrinterStatus> {
    let output = std::process::Command::new("lpstat")
        .args(["-p", name])
        .output();

    let output = match output {
        Ok(o) => o,
        Err(_) => return None,
    };

    if !output.status.success() || output.stdout.is_empty() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if line.starts_with("printer ") && line.contains(name) {
            if line.contains("disabled") {
                return Some(PrinterStatus::Offline);
            } else if line.contains("idle") || line.contains("enabled") {
                return Some(PrinterStatus::Online);
            }
        }
    }
    None
}

// ── supports_direct_pdf() ──────────────────────────────────────────

/// Detects if a CUPS printer supports native PDF rendering.
///
/// Fallback chain:
/// 1. `lpoptions -d {printer_name} -l` — look for pdftopdf / application/pdf
/// 2. PPD file at `/etc/cups/ppd/{printer_name}.ppd` — look for `*cupsFilter:* pdftopdf`
/// 3. Return `false` (safe default)
#[cfg(not(target_os = "windows"))]
fn detect_cups_direct_pdf(printer_name: &str) -> bool {
    if lpoptions_has_pdf_support(printer_name) {
        return true;
    }
    if ppd_has_pdf_filter(printer_name) {
        return true;
    }
    false
}

#[cfg(not(target_os = "windows"))]
fn lpoptions_has_pdf_support(printer_name: &str) -> bool {
    let output = std::process::Command::new("lpoptions")
        .args(["-d", printer_name, "-l"])
        .output();

    let output = match output {
        Ok(o) => o,
        Err(_) => return false,
    };

    if !output.status.success() {
        return false;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    parse_lpoptions_for_pdf(&text)
}

fn parse_lpoptions_for_pdf(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("pdftopdf") || lower.contains("application/pdf")
}

#[cfg(not(target_os = "windows"))]
fn ppd_has_pdf_filter(printer_name: &str) -> bool {
    let paths = [format!("/etc/cups/ppd/{}.ppd", printer_name), {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{}/.cups/ppd/{}.ppd", home, printer_name)
    }];

    for path in &paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            if parse_ppd_for_pdf_filter(&content) {
                return true;
            }
        }
    }
    false
}

fn parse_ppd_for_pdf_filter(content: &str) -> bool {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("*cupsFilter:") || trimmed.starts_with("*CupsFilter:") {
            let lower = trimmed.to_lowercase();
            if lower.contains("pdftopdf") {
                return true;
            }
        }
    }
    false
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lpstat_parse_enabled() {
        let output = "printer HP_LaserJet is idle.  enabled since Mon Jun 23 00:00:00 2026";
        let printers = parse_lpstat_output(output);
        assert_eq!(printers.len(), 1);
        assert_eq!(printers[0].name().as_str(), "HP_LaserJet");
        assert_eq!(printers[0].status(), &PrinterStatus::Online);
    }

    #[test]
    fn test_lpstat_parse_disabled() {
        let output = "printer Canon disabled since Mon Jun 23 00:00:00 2026 -\n\treason unknown";
        let printers = parse_lpstat_output(output);
        assert_eq!(printers.len(), 1);
        assert_eq!(printers[0].name().as_str(), "Canon");
        assert_eq!(printers[0].status(), &PrinterStatus::Offline);
    }

    #[test]
    fn test_printers_conf_parse_idle() {
        let conf = "<Printer TestPrinter>\nState Idle\nStateMessage\n</Printer>";
        let printers = parse_printers_conf(conf);
        assert_eq!(printers.len(), 1);
        assert_eq!(printers[0].name().as_str(), "TestPrinter");
        assert_eq!(printers[0].status(), &PrinterStatus::Online);
    }

    #[test]
    fn test_printers_conf_parse_stopped() {
        let conf = "<Printer OldPrinter>\nState Stopped\n</Printer>";
        let printers = parse_printers_conf(conf);
        assert_eq!(printers.len(), 1);
        assert_eq!(printers[0].name().as_str(), "OldPrinter");
        assert_eq!(printers[0].status(), &PrinterStatus::Offline);
    }

    #[test]
    fn test_discover_returns_vec() {
        let manager = CupsPrinterManager::new();
        let _printers = manager.discover_printers();
        // Must not panic — may be empty in CI without CUPS
    }

    #[test]
    fn test_get_status_returns_offline_when_not_found() {
        let manager = CupsPrinterManager::new();
        let status = manager.get_status("NonExistentPrinter_XYZ_12345");
        assert_eq!(status, PrinterStatus::Offline);
    }

    #[test]
    fn test_parse_lpoptions_with_pdftopdf() {
        let output =
            "Option1/Value1\n*cupsFilter: application/vnd.cups-postscript pdftopdf\nOption2/Value2";
        assert!(parse_lpoptions_for_pdf(output));
    }

    #[test]
    fn test_parse_lpoptions_with_application_pdf() {
        let output = "MediaType/plain application/pdf\nOtherOption/value";
        assert!(parse_lpoptions_for_pdf(output));
    }

    #[test]
    fn test_parse_lpoptions_no_pdf() {
        let output = "Option1/Value1\nOption2/Value2\nPageSize/Letter";
        assert!(!parse_lpoptions_for_pdf(output));
    }

    #[test]
    fn test_parse_ppd_with_pdftopdf_filter() {
        let ppd = "*cupsFilter: application/vnd.cups-pdf 0 pdftopdf\n*PageSize Letter/Letter";
        assert!(parse_ppd_for_pdf_filter(ppd));
    }

    #[test]
    fn test_parse_ppd_no_pdf_filter() {
        let ppd =
            "*cupsFilter: application/vnd.cups-postscript 0 rastertohp\n*PageSize Letter/Letter";
        assert!(!parse_ppd_for_pdf_filter(ppd));
    }

    #[test]
    fn test_supports_direct_pdf_nonexistent_printer() {
        let manager = CupsPrinterManager::new();
        assert!(
            !manager.supports_direct_pdf("NonExistentPrinter_XYZ_99999"),
            "Non-existent printer must return false"
        );
    }
}
