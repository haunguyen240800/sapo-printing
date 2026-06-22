use crate::domain::print_job::value_objects::PaperSize;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrintJobSettings {
    pub paper_size: PaperSize,
    pub orientation: String,
    pub margin_left: f64,
    pub margin_right: f64,
    pub margin_top: f64,
    pub margin_bottom: f64,
    pub color_mode: String,
    pub print_as_image: bool,
    pub dpi: u32,
    pub copies: u32,
    pub rotate: f32,
}

impl Default for PrintJobSettings {
    fn default() -> Self {
        Self {
            paper_size: PaperSize::A4,
            orientation: "PORTRAIT".to_string(),
            margin_left: 0.0,
            margin_right: 0.0,
            margin_top: 0.0,
            margin_bottom: 0.0,
            color_mode: "RGB".to_string(),
            print_as_image: false,
            dpi: 300,
            copies: 1,
            rotate: 0.0,
        }
    }
}
