use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterConfigResponse {
    pub printer_name: String,
    pub paper_size: String,          // "A4" | "A5" | "Letter" | "Custom"
    pub paper_width: Option<f64>,    // mm, required when paper_size = "Custom"
    pub paper_height: Option<f64>,   // mm, required when paper_size = "Custom"
    pub orientation: String,         // "Portrait" | "Landscape"
    pub margin_left: f64,            // mm
    pub margin_right: f64,           // mm
    pub margin_top: f64,             // mm
    pub margin_bottom: f64,          // mm
    pub print_as_image: bool,        // true = render as image before printing
    pub color_mode: String,          // "RGB" | "ARGB" | "BGR" | "GRAY" | "BINARY"
    pub enable_buffer: bool,         // true = enable printing buffer
    pub buffer_size_kb: Option<f64>, // KB, 1-1024, required when enable_buffer = true
}
