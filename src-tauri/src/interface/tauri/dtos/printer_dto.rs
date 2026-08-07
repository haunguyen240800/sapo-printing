use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterDto {
    pub name: String,
    pub device_id: String,
    pub status: String,       // "Online" | "Offline" | "Error"
    pub printer_type: String, // "Local" | "Network"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_default: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterConfigDto {
    pub printer_name: String,
    pub paper_size: String,          // "A4" | "A5" | "Letter" | "Custom"
    pub paper_width: Option<u32>,    // mm, required when paper_size = "Custom"
    pub paper_height: Option<u32>,   // mm, required when paper_size = "Custom"
    pub orientation: String,         // "Portrait" | "Landscape"
    pub margin_left: u32,            // mm
    pub margin_right: u32,           // mm
    pub margin_top: u32,             // mm
    pub margin_bottom: u32,          // mm
    pub print_as_image: bool,        // true = render as image before printing
    pub color_mode: String,          // "RGB" | "ARGB" | "BGR" | "GRAY" | "BINARY"
    pub enable_buffer: bool,         // true = enable printing buffer
    pub buffer_size_kb: Option<u32>, // KB, 1-1024, required when enable_buffer = true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterStatusDto {
    pub status: String, // "Online" | "Offline" | "Error"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_printer_dto_serialization() {
        let dto = PrinterDto {
            name: "HP LaserJet".to_string(),
            device_id: "hp-laser-001".to_string(),
            status: "Online".to_string(),
            printer_type: "Local".to_string(),
            is_default: Some(true),
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("HP LaserJet"));
        assert!(json.contains("Online"));
    }

    #[test]
    fn test_printer_dto_deserialization() {
        let json = r#"{
            "name": "Canon Printer",
            "device_id": "canon-001",
            "status": "Offline",
            "printer_type": "Network",
            "is_default": false
        }"#;
        let dto: PrinterDto = serde_json::from_str(json).unwrap();
        assert_eq!(dto.name, "Canon Printer");
        assert_eq!(dto.status, "Offline");
        assert_eq!(dto.is_default, Some(false));
    }

    #[test]
    fn test_printer_config_dto_serialization() {
        let config = PrinterConfigDto {
            printer_name: "HP LaserJet".to_string(),
            paper_size: "A4".to_string(),
            paper_width: None,
            paper_height: None,
            orientation: "Portrait".to_string(),
            margin_left: 10,
            margin_right: 10,
            margin_top: 15,
            margin_bottom: 15,
            print_as_image: false,
            color_mode: "RGB".to_string(),
            enable_buffer: false,
            buffer_size_kb: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("A4"));
        assert!(json.contains("Portrait"));
    }

    #[test]
    fn test_printer_config_dto_with_custom_paper() {
        let config = PrinterConfigDto {
            printer_name: "Custom Printer".to_string(),
            paper_size: "Custom".to_string(),
            paper_width: Some(200),
            paper_height: Some(300),
            orientation: "Landscape".to_string(),
            margin_left: 5,
            margin_right: 5,
            margin_top: 5,
            margin_bottom: 5,
            print_as_image: true,
            color_mode: "ARGB".to_string(),
            enable_buffer: true,
            buffer_size_kb: Some(512),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: PrinterConfigDto = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.paper_width, Some(200));
        assert_eq!(parsed.paper_height, Some(300));
        assert_eq!(parsed.print_as_image, true);
        assert_eq!(parsed.buffer_size_kb, Some(512));
    }

    #[test]
    fn test_printer_status_dto_serialization() {
        let status = PrinterStatusDto {
            status: "Error".to_string(),
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("Error"));
    }
}
