//! `PaperSize` — value object representing the physical paper a job is printed on.
//!
//! Replaces the previous primitive obsession of three separate fields
//! (`paper_size: String`, `paper_width: Option<u32>`, `paper_height: Option<u32>`)
//! with a single algebraic data type. Standard sizes carry no dimensions
//! (LayoutEngine resolves them); `Custom` carries explicit mm dimensions.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "UPPERCASE")]
pub enum PaperSize {
    // Standard sizes
    A4,
    A5,
    Letter,
    // SAPO label sizes (matches frontend PaperSizeOptions values)
    Cm10x10,
    Cm10x12,
    Cm10x15,
    Cm10x18,
    // Thermal roll sizes (legacy, kept for backward compat)
    K80,
    K58,
    // Fully user-defined
    Custom { width_mm: f64, height_mm: f64 },
}

impl PaperSize {
    /// Physical dimensions in millimetres (width, height) at portrait orientation.
    pub fn dimensions_mm(&self) -> (f32, f32) {
        match self {
            PaperSize::A4 => (210.0, 297.0),
            PaperSize::A5 => (148.0, 210.0),
            PaperSize::Letter => (216.0, 279.0),
            PaperSize::Cm10x10 => (100.0, 100.0),
            PaperSize::Cm10x12 => (100.0, 120.0),
            PaperSize::Cm10x15 => (100.0, 150.0),
            PaperSize::Cm10x18 => (100.0, 180.0),
            PaperSize::K80 => (80.0, 297.0),
            PaperSize::K58 => (58.0, 297.0),
            PaperSize::Custom {
                width_mm,
                height_mm,
            } => (*width_mm as f32, *height_mm as f32),
        }
    }

    /// Parse from a (size_name, optional width, optional height) triple.
    /// Used at the boundary with persisted config (which still stores fields separately).
    pub fn from_parts(name: &str, width_mm: Option<f64>, height_mm: Option<f64>) -> Self {
        match name.to_uppercase().as_str() {
            "A4" => PaperSize::A4,
            "A5" => PaperSize::A5,
            "LETTER" => PaperSize::Letter,
            "CM10X10" => PaperSize::Cm10x10,
            "CM10X12" => PaperSize::Cm10x12,
            "CM10X15" => PaperSize::Cm10x15,
            "CM10X18" => PaperSize::Cm10x18,
            "K80" => PaperSize::K80,
            "K58" => PaperSize::K58,
            "CUSTOM" => PaperSize::Custom {
                width_mm: width_mm.unwrap_or(210.0),
                height_mm: height_mm.unwrap_or(297.0),
            },
            // Unknown name with explicit dimensions → treat as custom size.
            _ => match (width_mm, height_mm) {
                (Some(w), Some(h)) => PaperSize::Custom {
                    width_mm: w,
                    height_mm: h,
                },
                _ => PaperSize::A4,
            },
        }
    }

    pub fn as_name(&self) -> &'static str {
        match self {
            PaperSize::A4 => "A4",
            PaperSize::A5 => "A5",
            PaperSize::Letter => "Letter",
            PaperSize::Cm10x10 => "CM10x10",
            PaperSize::Cm10x12 => "CM10x12",
            PaperSize::Cm10x15 => "CM10x15",
            PaperSize::Cm10x18 => "CM10x18",
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
