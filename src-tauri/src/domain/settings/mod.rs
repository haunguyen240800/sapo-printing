use crate::domain::common::value_object::ValueObject;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrintSettings {
    pub paper_size: String,
    pub paper_width: Option<u32>,
    pub paper_height: Option<u32>,
    pub orientation: String,
    pub margin_left: u32,
    pub margin_right: u32,
    pub margin_top: u32,
    pub margin_bottom: u32,
    pub color_mode: String,
    pub print_as_image: bool,
    pub dpi: u32,
    pub copies: u32,
    pub rotate: f32,
}

impl Default for PrintSettings {
    fn default() -> Self {
        Self {
            paper_size: "A4".to_string(),
            paper_width: None,
            paper_height: None,
            orientation: "PORTRAIT".to_string(),
            margin_left: 0,
            margin_right: 0,
            margin_top: 0,
            margin_bottom: 0,
            color_mode: "RGB".to_string(),
            print_as_image: false,
            dpi: 300,
            copies: 1,
            rotate: 0.0,
        }
    }
}

impl ValueObject for PrintSettings {}
