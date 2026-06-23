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
pub fn save_printer_config(config: PrinterConfigDto) -> Result<(), String> {
    // Validate buffer settings
    if config.enable_buffer {
        match config.buffer_size_kb {
            Some(size) if (1..=1024).contains(&size) => {
                // Valid buffer size
            }
            Some(size) => {
                return Err(format!(
                    "Kích thước buffer phải trong khoảng 1-1024 KB (nhận được: {} KB)",
                    size
                ));
            }
            None => {
                return Err("Kích thước buffer bắt buộc khi bật buffer".to_string());
            }
        }
    } else if config.buffer_size_kb.is_some() {
        return Err("Không thể đặt kích thước buffer khi buffer đã tắt".to_string());
    }

    // Validate color mode
    let valid_color_modes = ["RGB", "ARGB", "BGR", "GRAY", "BINARY"];
    if !valid_color_modes.contains(&config.color_mode.as_str()) {
        return Err(format!(
            "Loại ảnh in không hợp lệ: '{}'. Chỉ chấp nhận: RGB, ARGB, BGR, GRAY, BINARY",
            config.color_mode
        ));
    }

    // TODO: Implement persistence in Story 2.6
    // 1. Load or create Printer from repository
    // 2. Update printer config fields (including new fields)
    // 3. Call printer_repository.save()
    // 4. Return Ok(()) or Vietnamese error message
    Err("Chưa triển khai lưu cấu hình".to_string())
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
            print_as_image: false,
            color_mode: "RGB".to_string(),
            enable_buffer: false,
            buffer_size_kb: None,
        };
        let result = save_printer_config(config);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Chưa triển khai lưu cấu hình");
    }

    #[test]
    fn test_save_printer_config_validates_buffer_size_required() {
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
            print_as_image: false,
            color_mode: "RGB".to_string(),
            enable_buffer: true,
            buffer_size_kb: None, // Invalid: buffer enabled but no size
        };
        let result = save_printer_config(config);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Kích thước buffer bắt buộc khi bật buffer");
    }

    #[test]
    fn test_save_printer_config_validates_buffer_size_range() {
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
            print_as_image: false,
            color_mode: "RGB".to_string(),
            enable_buffer: true,
            buffer_size_kb: Some(2048), // Invalid: exceeds max 1024
        };
        let result = save_printer_config(config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("1-1024 KB"));
    }

    #[test]
    fn test_save_printer_config_rejects_buffer_size_when_disabled() {
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
            print_as_image: false,
            color_mode: "RGB".to_string(),
            enable_buffer: false,
            buffer_size_kb: Some(512), // Invalid: buffer disabled but size provided
        };
        let result = save_printer_config(config);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Không thể đặt kích thước buffer khi buffer đã tắt");
    }

    #[test]
    fn test_save_printer_config_validates_color_mode() {
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
            print_as_image: true,
            color_mode: "INVALID_MODE".to_string(), // Invalid color mode
            enable_buffer: false,
            buffer_size_kb: None,
        };
        let result = save_printer_config(config);
        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("Loại ảnh in không hợp lệ"));
        assert!(err_msg.contains("RGB, ARGB, BGR, GRAY, BINARY"));
    }

    #[test]
    fn test_get_printer_status_stub_returns_error() {
        let result = get_printer_status("HP LaserJet".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Không tìm thấy máy in"));
    }
}
