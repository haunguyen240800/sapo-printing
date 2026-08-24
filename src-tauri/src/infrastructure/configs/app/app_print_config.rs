use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPrintConfig {
    pub printer_name: String,
    pub paper_size: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paper_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paper_height: Option<f64>,
    pub orientation: String,
    pub margin_left: f64,
    pub margin_right: f64,
    pub margin_top: f64,
    pub margin_bottom: f64,
    pub color_mode: String,
    pub print_as_image: bool,
    pub enable_buffer: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buffer_size_kb: Option<f64>,
}

impl Default for AppPrintConfig {
    fn default() -> Self {
        Self {
            printer_name: String::new(),
            paper_size: "A4".to_string(),
            paper_width: None,
            paper_height: None,
            orientation: "Portrait".to_string(),
            margin_left: 0.0,
            margin_right: 0.0,
            margin_top: 0.0,
            margin_bottom: 0.0,
            color_mode: "RGB".to_string(),
            print_as_image: false,
            enable_buffer: false,
            buffer_size_kb: None,
        }
    }
}

pub fn save_config(config_path: &Path, config: &AppPrintConfig) -> Result<(), String> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Không thể tạo thư mục cấu hình {}: {}", parent.display(), e))?;
    }

    let json = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Không thể serialize cấu hình: {}", e))?;

    fs::write(config_path, json)
        .map_err(|e| format!("Không thể lưu cấu hình {}: {}", config_path.display(), e))?;

    tracing::info!(
        target = "sapo_printer::config_store",
        path = ?config_path,
        "Print config saved to file"
    );

    Ok(())
}

pub fn load_config(config_path: &Path) -> Result<Option<AppPrintConfig>, String> {
    if !config_path.exists() {
        tracing::debug!(
            target = "sapo_printer::config_store",
            path = ?config_path,
            "Config file does not exist, returning None"
        );
        return Ok(None);
    }

    let json = fs::read_to_string(config_path)
        .map_err(|e| format!("Không thể đọc cấu hình {}: {}", config_path.display(), e))?;

    let config: AppPrintConfig = serde_json::from_str(&json)
        .map_err(|e| format!("Không thể parse cấu hình {}: {}", config_path.display(), e))?;

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
        assert_eq!(config.margin_left, 0.0);
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
        assert_eq!(config.margin_left, 10.0);
    }

    #[test]
    fn save_and_load_config_at_injected_path() {
        let root = std::env::temp_dir().join(format!("sapo_config_{}", uuid::Uuid::new_v4()));
        let config_path = root.join("nested").join("print-config.json");
        let mut expected = AppPrintConfig::default();
        expected.printer_name = "Injected Printer".to_string();

        save_config(&config_path, &expected).unwrap();
        let loaded = load_config(&config_path).unwrap().unwrap();

        assert_eq!(loaded.printer_name, expected.printer_name);
        assert_eq!(loaded.paper_size, expected.paper_size);
        assert!(config_path.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn load_missing_config_does_not_create_parent_directory() {
        let root = std::env::temp_dir().join(format!("sapo_config_{}", uuid::Uuid::new_v4()));
        let config_path = root.join("print-config.json");

        assert!(load_config(&config_path).unwrap().is_none());
        assert!(!root.exists());
    }
}
