use crate::interface::tauri::dtos::printer_dto::{PrinterConfigDto, PrinterDto, PrinterStatusDto};

/// List all available printers (discovered + saved configs)
///
/// # Returns
/// - `Ok(Vec<PrinterDto>)` with printer list (can be empty)
/// - `Err(String)` on failure
#[tauri::command]
pub fn list_printers() -> Result<Vec<PrinterDto>, String> {
    // TODO: Implement in Task 2
    // 1. Access PrinterManager from AppContext
    // 2. Call discover_printers()
    // 3. Load saved configs from PrinterRepository
    // 4. Merge discovered printers with configs (is_default flag)
    // 5. Map to PrinterDto
    Ok(vec![])
}

/// Save printer configuration
///
/// # Arguments
/// - `config`: PrinterConfigDto with all settings
///
/// # Returns
/// - `Ok(())` on success
/// - `Err(String)` with Vietnamese error message on validation or save failure
#[tauri::command]
pub fn save_printer_config(_config: PrinterConfigDto) -> Result<(), String> {
    // TODO: Implement in Task 3
    // 1. Validate config (paper size, custom dimensions 50-500mm, margins 0-100mm)
    // 2. Load or create Printer from repository
    // 3. Update printer config fields
    // 4. Call printer_repository.save()
    // 5. Return Ok(()) or Vietnamese error message
    Err("Chưa triển khai".to_string())
}

/// Get current printer status
///
/// # Arguments
/// - `name`: Printer name to query
///
/// # Returns
/// - `Ok(PrinterStatusDto)` with status string
/// - `Err(String)` if printer not found or query fails
#[tauri::command]
pub fn get_printer_status(name: String) -> Result<PrinterStatusDto, String> {
    // TODO: Implement in Task 4
    // 1. Access PrinterManager from AppContext
    // 2. Call printer_manager.get_status(name)
    // 3. Map PrinterStatus enum to String
    // 4. Return PrinterStatusDto
    Err(format!("Không tìm thấy máy in: {}", name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_printers_stub_returns_empty() {
        let result = list_printers();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_save_printer_config_stub_returns_error() {
        let config = PrinterConfigDto {
            printer_name: "Test Printer".to_string(),
            paper_size: "A4".to_string(),
            paper_width: None,
            paper_height: None,
            orientation: "Portrait".to_string(),
            margin_left: 10,
            margin_right: 10,
            margin_top: 10,
            margin_bottom: 10,
        };
        let result = save_printer_config(config);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Chưa triển khai");
    }

    #[test]
    fn test_get_printer_status_stub_returns_error() {
        let result = get_printer_status("HP LaserJet".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Không tìm thấy máy in"));
    }
}
