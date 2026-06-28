//! `PaperSize` — value object representing the physical paper a job is printed on.
//!
//! Replaces the previous primitive obsession of three separate fields
//! (`paper_size: String`, `paper_width: Option<u32>`, `paper_height: Option<u32>`)
//! with a single algebraic data type. Standard sizes carry no dimensions
//! (LayoutEngine resolves them); `Custom` carries explicit mm dimensions.

use serde::{Deserialize, Serialize};

use crate::domain::common::value_object::ValueObject;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "UPPERCASE")]
pub enum PaperSize {
    A4,
    A5,
    Letter,
    K80,
    K58,
    Custom { width_mm: u32, height_mm: u32 },
}

impl PaperSize {
    /// Physical dimensions in millimetres (width, height) at portrait orientation.
    pub fn dimensions_mm(&self) -> (f32, f32) {
        match self {
            PaperSize::A4 => (210.0, 297.0),
            PaperSize::A5 => (148.0, 210.0),
            PaperSize::Letter => (215.9, 279.4),
            PaperSize::K80 => (80.0, 297.0),
            PaperSize::K58 => (58.0, 297.0),
            PaperSize::Custom { width_mm, height_mm } => (*width_mm as f32, *height_mm as f32),
        }
    }

    /// Parse from a (size_name, optional width, optional height) triple.
    /// Used at the boundary with persisted config (which still stores fields separately).
    pub fn from_parts(name: &str, width_mm: Option<u32>, height_mm: Option<u32>) -> Self {
        match name.to_uppercase().as_str() {
            "A4" => PaperSize::A4,
            "A5" => PaperSize::A5,
            "LETTER" => PaperSize::Letter,
            "K80" => PaperSize::K80,
            "K58" => PaperSize::K58,
            "CUSTOM" => PaperSize::Custom {
                width_mm: width_mm.unwrap_or(210),
                height_mm: height_mm.unwrap_or(297),
            },
            _ => PaperSize::A4,
        }
    }

    pub fn as_name(&self) -> &'static str {
        match self {
            PaperSize::A4 => "A4",
            PaperSize::A5 => "A5",
            PaperSize::Letter => "Letter",
            PaperSize::K80 => "K80",
            PaperSize::K58 => "K58",
            PaperSize::Custom { .. } => "Custom",
        }
    }
}

impl Default for PaperSize {
    fn default() -> Self {
        PaperSize::A4
    }
}

impl ValueObject for PaperSize {}
