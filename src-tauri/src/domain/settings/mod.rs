use crate::domain::common::value_object::ValueObject;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrintSettings {
    pub paper_size: String,
    pub orientation: String,
    pub margin: f32,
    pub scale_mode: String,
    pub dpi: u32,
    pub grayscale: bool,
    pub binary: bool,
    pub copies: u32,
    pub rotate: f32,
}

impl Default for PrintSettings {
    fn default() -> Self {
        Self {
            paper_size: "A4".to_string(),
            orientation: "PORTRAIT".to_string(),
            margin: 0.0,
            scale_mode: "FIT".to_string(),
            dpi: 300,
            grayscale: false,
            binary: false,
            copies: 1,
            rotate: 0.0,
        }
    }
}

impl ValueObject for PrintSettings {}
