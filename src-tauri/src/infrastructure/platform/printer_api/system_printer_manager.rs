//! `SystemPrinterManager` — OS-backed adapter implementing the `PrinterManager` port.
//!
//! Single source of truth for OS printer enumeration and availability checks.
//! Uses platform commands (PowerShell on Windows, `lpstat` on Unix) to query
//! the spooler and maps the raw shape into the application's `PrinterDto`.
//!
//! Results are cached in-process for a short TTL ([`LIST_CACHE_TTL`]) because
//! each list call spawns a subprocess (~300ms–1s) and the UI tends to refresh
//! the list rapidly.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::application::dto::PrinterDto;
use crate::application::ports::{PrinterAvailability, PrinterManager};
use crate::shared::errors::InfrastructureError;

/// How long a successful `list()` result is reused before re-querying the OS.
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

    /// Return the cached printer list if it is still fresh, otherwise query
    /// the OS and refresh the cache.
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
fn list_os_printers() -> Result<Vec<PrinterDto>, InfrastructureError> {
    use serde_json::Value;
    use std::process::Command;

    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            r#"
$default = (Get-WmiObject -Class Win32_Printer | Where-Object { $_.Default -eq $true }).Name
Get-Printer | Select-Object Name, PrinterStatus, @{Name='IsDefault';Expression={ $_.Name -eq $default }} | ConvertTo-Json -Depth 2
"#,
        ])
        .output()
        .map_err(|e| InfrastructureError::PrinterError {
            reason: format!("Failed to execute powershell: {}", e),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(InfrastructureError::PrinterError {
            reason: format!("PowerShell command failed: {}", stderr),
        });
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    if json_str.trim().is_empty() {
        return Ok(vec![]);
    }

    let parsed: Value = serde_json::from_str(&json_str).map_err(|e| {
        InfrastructureError::PrinterError {
            reason: format!("Failed to parse printer JSON: {}", e),
        }
    })?;

    let items = match parsed {
        Value::Array(arr) => arr,
        Value::Object(_) => vec![parsed.clone()],
        _ => return Ok(vec![]),
    };

    let mut printers = Vec::new();
    for item in items {
        if let Some(name) = item.get("Name").and_then(|v| v.as_str()) {
            // PrinterStatus values from Win32:
            // 1=Other, 2=Unknown, 3=Idle(Online), 4=Printing, 5=WarmUp
            // 6=StoppedPrinting(Error), 7=Offline
            let status = match item.get("PrinterStatus").and_then(|v| v.as_u64()) {
                Some(3) | Some(4) | Some(5) => "Online",
                Some(7) => "Offline",
                Some(6) => "Error",
                _ => "Unknown",
            };
            let is_default = item
                .get("IsDefault")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            printers.push(PrinterDto {
                id: name.to_string(),
                name: name.to_string(),
                status: status.to_string(),
                printer_type: "Local".to_string(),
                is_default,
            });
        }
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
