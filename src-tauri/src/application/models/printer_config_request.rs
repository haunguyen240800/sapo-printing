use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterConfigRequest {
    pub printer_name: String,
    pub paper_size: String,          // "A4" | "A5" | "Letter" | "Custom"
    pub paper_width: u32,            // mm, required when paper_size = "Custom" (validated as not null)
    pub paper_height: u32,           // mm, required when paper_size = "Custom" (validated as not null)
    pub orientation: String,         // "Portrait" | "Landscape"
    pub margin_left: u32,            // mm
    pub margin_right: u32,           // mm
    pub margin_top: u32,             // mm
    pub margin_bottom: u32,          // mm
    pub print_as_image: bool,        // true = render as image before printing
    pub color_mode: String,          // "RGB" | "ARGB" | "BGR" | "GRAY" | "BINARY"
    pub enable_buffer: bool,         // true = enable printing buffer
    pub buffer_size_kb: u32,         // KB, 1-1024, required when enable_buffer = true (validated as not null)
}
