use crate::domain::models::printer::Printer;
use crate::domain::repository::printer_discovery::PrinterDiscovery;

pub struct SystemPrinterDiscovery;

impl SystemPrinterDiscovery {
    pub fn new() -> Self {
        Self {}
    }
}

impl PrinterDiscovery for SystemPrinterDiscovery {
    fn list_printers(&self) -> Result<Vec<Printer>, String> {
        #[cfg(target_os = "windows")]
        {
            use std::process::Command;
            use serde_json::Value;

            // Query printers including PrinterStatus (integer) and Default flag
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
                .map_err(|e| format!("Failed to execute powershell: {}", e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("PowerShell command failed: {}", stderr));
            }

            let json_str = String::from_utf8_lossy(&output.stdout);
            if json_str.trim().is_empty() {
                return Ok(vec![]);
            }

            let mut printers = Vec::new();
            let parsed: Value = serde_json::from_str(&json_str)
                .map_err(|e| format!("Failed to parse printer JSON: {}", e))?;

            let items = match parsed {
                Value::Array(arr) => arr,
                Value::Object(_) => vec![parsed.clone()], // single printer case
                _ => return Ok(vec![]),
            };

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

                    printers.push(Printer {
                        name: name.to_string(),
                        device_id: name.to_string(),
                        status: status.to_string(),
                        printer_type: "Local".to_string(),
                        is_default: Some(is_default),
                    });
                }
            }
            return Ok(printers);
        }

        #[cfg(not(target_os = "windows"))]
        {
            use std::process::Command;

            // Get default printer name from lpstat -d
            let default_printer = Command::new("lpstat")
                .arg("-d")
                .output()
                .ok()
                .and_then(|o| {
                    let text = String::from_utf8_lossy(&o.stdout).to_string();
                    // Format: "system default destination: <name>"
                    text.split(':').nth(1).map(|s| s.trim().to_string())
                })
                .unwrap_or_default();

            let output = Command::new("lpstat")
                .arg("-p")
                .output()
                .map_err(|e| format!("Failed to execute lpstat: {}", e))?;

            if !output.status.success() {
                return Ok(vec![]); // Fallback to empty if CUPS is not running
            }

            let text = String::from_utf8_lossy(&output.stdout);
            let mut printers = Vec::new();

            for line in text.lines() {
                if line.starts_with("printer ") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let name = parts[1].to_string();
                        // lpstat -p line: "printer <name> is idle. enabled since ..."
                        //               "printer <name> disabled since ..."
                        let status = if line.contains("idle") || line.contains("printing") {
                            "Online"
                        } else if line.contains("disabled") {
                            "Offline"
                        } else {
                            "Unknown"
                        };

                        let is_default = name == default_printer;

                        printers.push(Printer {
                            name: name.clone(),
                            device_id: name,
                            status: status.to_string(),
                            printer_type: "Local".to_string(),
                            is_default: Some(is_default),
                        });
                    }
                }
            }
            return Ok(printers);
        }
    }
}
