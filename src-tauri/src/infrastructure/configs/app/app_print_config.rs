//! Configuration Store Module
//!
//! Handles reading and writing application print configuration to JSON file.
//! File location: ~/.sapo-printer/print-config.json (or %APPDATA%/sapo-printer on Windows)

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Print configuration stored in JSON file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPrintConfig {
    pub printer_name: String,
    pub paper_size: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paper_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paper_height: Option<u32>,
    pub orientation: String,
    pub margin_left: u32,
    pub margin_right: u32,
    pub margin_top: u32,
    pub margin_bottom: u32,
    pub color_mode: String,
    pub print_as_image: bool,
    pub enable_buffer: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buffer_size_kb: Option<u32>,
}

impl Default for AppPrintConfig {
    fn default() -> Self {
        Self {
            printer_name: String::new(),
            paper_size: "A4".to_string(),
            paper_width: None,
            paper_height: None,
            orientation: "Portrait".to_string(),
            margin_left: 0,
            margin_right: 0,
            margin_top: 0,
            margin_bottom: 0,
            color_mode: "RGB".to_string(),
            print_as_image: false,
            enable_buffer: false,
            buffer_size_kb: None,
        }
    }
}

/// Get the path to the config file
fn get_config_file_path() -> Result<PathBuf, String> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| "Cannot determine home directory")?;

    let config_dir = PathBuf::from(&home).join(".sapo-printer");

    // Create directory if not exists
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Không thể tạo thư mục cấu hình: {}", e))?;
    }

    Ok(config_dir.join("print-config.json"))
}

/// Save print configuration to JSON file
pub fn save_config(config: &AppPrintConfig) -> Result<(), String> {
    let config_path = get_config_file_path()?;

    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Không thể serialize cấu hình: {}", e))?;

    fs::write(&config_path, json)
        .map_err(|e| format!("Không thể lưu cấu hình: {}", e))?;

    tracing::info!(
        target = "sapo_printer::config_store",
        path = ?config_path,
        "Print config saved to file"
    );

    Ok(())
}

/// Load print configuration from JSON file
pub fn load_config() -> Result<Option<AppPrintConfig>, String> {
    let config_path = get_config_file_path()?;

    if !config_path.exists() {
        tracing::debug!(
            target = "sapo_printer::config_store",
            path = ?config_path,
            "Config file does not exist, returning None"
        );
        return Ok(None);
    }

    let json = fs::read_to_string(&config_path)
        .map_err(|e| format!("Không thể đọc cấu hình: {}", e))?;

    let config: AppPrintConfig = serde_json::from_str(&json)
        .map_err(|e| format!("Không thể parse cấu hình: {}", e))?;

    tracing::info!(
        target = "sapo_printer::config_store",
        path = ?config_path,
        printer_name = %config.printer_name,
        "Print config loaded from file"
    );

    Ok(Some(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppPrintConfig::default();
        assert_eq!(config.paper_size, "A4");
        assert_eq!(config.orientation, "Portrait");
        assert_eq!(config.margin_left, 0);
    }

    #[test]
    fn test_serialize_config() {
        let config = AppPrintConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("A4"));
        assert!(json.contains("Portrait"));
    }

    #[test]
    fn test_deserialize_config() {
        let json = r#"{
            "printer_name": "Test Printer",
            "paper_size": "A4",
            "orientation": "Portrait",
            "margin_left": 10,
            "margin_right": 10,
            "margin_top": 10,
            "margin_bottom": 10,
            "color_mode": "RGB",
            "print_as_image": false,
            "enable_buffer": false
        }"#;

        let config: AppPrintConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.printer_name, "Test Printer");
        assert_eq!(config.paper_size, "A4");
        assert_eq!(config.margin_left, 10);
    }
}
