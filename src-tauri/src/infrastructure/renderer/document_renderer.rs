use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::shared::errors::InfrastructureError;

/// Color mode for rendered output.
///
/// Determines the pixel format of the bitmap produced by the renderer.
/// Each mode targets different printer capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorMode {
    /// 24-bit color (8 bits per channel: R, G, B) — 3 bytes/pixel
    Rgb,
    /// 32-bit color with alpha channel — 4 bytes/pixel
    Argb,
    /// Windows default byte order (B, G, R) — 3 bytes/pixel
    Bgr,
    /// 8-bit grayscale — 1 byte/pixel
    Gray,
    /// 1-bit monochrome — 1 bit/pixel (8 pixels per byte)
    Binary,
}

/// Paper size in millimeters.
///
/// Provides predefined sizes (A4, A5, Letter) and supports custom dimensions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaperSize {
    pub width_mm: f64,
    pub height_mm: f64,
}

impl PaperSize {
    pub fn a4() -> Self {
        Self {
            width_mm: 210.0,
            height_mm: 297.0,
        }
    }

    pub fn a5() -> Self {
        Self {
            width_mm: 148.0,
            height_mm: 210.0,
        }
    }

    pub fn letter() -> Self {
        Self {
            width_mm: 215.9,
            height_mm: 279.4,
        }
    }
}

/// Configuration for rendering a PDF document.
///
/// Controls margins, color mode, paper size, and DPI.
/// All margins are in millimeters and must be non-negative.
/// Combined left+right margins must be less than paper width.
/// Combined top+bottom margins must be less than paper height.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderConfig {
    pub margin_left_mm: f64,
    pub margin_right_mm: f64,
    pub margin_top_mm: f64,
    pub margin_bottom_mm: f64,
    pub color_mode: ColorMode,
    pub paper_size: PaperSize,
    pub dpi: u32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            margin_left_mm: 0.0,
            margin_right_mm: 0.0,
            margin_top_mm: 0.0,
            margin_bottom_mm: 0.0,
            color_mode: ColorMode::Rgb,
            paper_size: PaperSize::a4(),
            dpi: 300,
        }
    }
}

impl RenderConfig {
    /// Validates that margins are non-negative and fit within the paper size.
    pub fn validate(&self) -> Result<(), InfrastructureError> {
        if self.dpi == 0 {
            return Err(InfrastructureError::ValidationError(
                "DPI must be greater than zero".to_string(),
            ));
        }
        if self.margin_left_mm.is_nan()
            || self.margin_right_mm.is_nan()
            || self.margin_top_mm.is_nan()
            || self.margin_bottom_mm.is_nan()
            || self.paper_size.width_mm.is_nan()
            || self.paper_size.height_mm.is_nan()
        {
            return Err(InfrastructureError::ValidationError(
                "Margins and paper dimensions must be finite numbers".to_string(),
            ));
        }
        if self.margin_left_mm < 0.0
            || self.margin_right_mm < 0.0
            || self.margin_top_mm < 0.0
            || self.margin_bottom_mm < 0.0
        {
            return Err(InfrastructureError::ValidationError(
                "All margins must be non-negative".to_string(),
            ));
        }
        if self.margin_left_mm + self.margin_right_mm >= self.paper_size.width_mm {
            return Err(InfrastructureError::ValidationError(
                "Left + right margins must be less than paper width".to_string(),
            ));
        }
        if self.margin_top_mm + self.margin_bottom_mm >= self.paper_size.height_mm {
            return Err(InfrastructureError::ValidationError(
                "Top + bottom margins must be less than paper height".to_string(),
            ));
        }
        Ok(())
    }
}

/// Trait for rendering PDF documents to bitmap.
///
/// Concrete implementations handle DPI scaling, margin application,
/// and color mode conversion. Used by QueueWorker (Story 3.5) and
/// selected by StrategySelector (Story 3.4).
///
/// # Errors
/// - `ValidationError`: invalid or corrupt PDF file
/// - `RenderError`: PDFium rendering failure
/// - `TimeoutError`: render exceeds 5s per page
pub trait DocumentRenderer: Send + Sync {
    fn render(&self, path: &Path, config: &RenderConfig) -> Result<Vec<u8>, InfrastructureError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RenderConfig::default();
        assert_eq!(config.margin_left_mm, 0.0);
        assert_eq!(config.margin_right_mm, 0.0);
        assert_eq!(config.margin_top_mm, 0.0);
        assert_eq!(config.margin_bottom_mm, 0.0);
        assert_eq!(config.color_mode, ColorMode::Rgb);
        assert_eq!(config.paper_size, PaperSize::a4());
        assert_eq!(config.dpi, 300);
    }

    #[test]
    fn test_paper_sizes() {
        let a4 = PaperSize::a4();
        assert_eq!(a4.width_mm, 210.0);
        assert_eq!(a4.height_mm, 297.0);

        let a5 = PaperSize::a5();
        assert_eq!(a5.width_mm, 148.0);
        assert_eq!(a5.height_mm, 210.0);

        let letter = PaperSize::letter();
        assert_eq!(letter.width_mm, 215.9);
        assert_eq!(letter.height_mm, 279.4);
    }

    #[test]
    fn test_validate_config_ok() {
        let config = RenderConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_config_negative_margin() {
        let config = RenderConfig {
            margin_left_mm: -1.0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_config_margins_exceed_width() {
        let config = RenderConfig {
            margin_left_mm: 110.0,
            margin_right_mm: 110.0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_config_margins_exceed_height() {
        let config = RenderConfig {
            margin_top_mm: 150.0,
            margin_bottom_mm: 150.0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_color_mode_serialization() {
        let mode = ColorMode::Rgb;
        let json = serde_json::to_string(&mode).unwrap();
        let deserialized: ColorMode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ColorMode::Rgb);
    }

    #[test]
    fn test_render_config_serialization() {
        let config = RenderConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RenderConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.dpi, config.dpi);
        assert_eq!(deserialized.color_mode, config.color_mode);
    }

    #[test]
    fn test_validate_config_dpi_zero() {
        let config = RenderConfig {
            dpi: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_config_nan_margin() {
        let config = RenderConfig {
            margin_left_mm: f64::NAN,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_config_nan_paper_size() {
        let config = RenderConfig {
            paper_size: PaperSize {
                width_mm: f64::NAN,
                height_mm: 297.0,
            },
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }
}
