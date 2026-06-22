/// Convert millimeters to pixels at given DPI
pub fn mm_to_pixels(mm: f64, dpi: u32) -> u32 {
    (mm / 25.4 * dpi as f64).round() as u32
}

/// Convert pixels to millimeters at given DPI
///
/// # Panics
/// Panics if `dpi` is zero.
pub fn pixels_to_mm(pixels: u32, dpi: u32) -> f64 {
    assert!(dpi > 0, "dpi must be greater than zero");
    pixels as f64 / dpi as f64 * 25.4
}

/// Convert millimeters to inches
pub fn mm_to_inches(mm: f64) -> f64 {
    mm / 25.4
}

/// Convert inches to millimeters
pub fn inches_to_mm(inches: f64) -> f64 {
    inches * 25.4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mm_to_pixels_a4_width() {
        assert_eq!(mm_to_pixels(210.0, 300), 2480);
    }

    #[test]
    fn test_mm_to_pixels_a4_height() {
        assert_eq!(mm_to_pixels(297.0, 300), 3508);
    }

    #[test]
    fn test_pixels_to_mm_roundtrip() {
        let result = pixels_to_mm(mm_to_pixels(210.0, 300), 300);
        assert!((result - 210.0).abs() < 0.5);
    }

    #[test]
    fn test_mm_to_inches() {
        assert_eq!(mm_to_inches(25.4), 1.0);
    }

    #[test]
    fn test_inches_to_mm() {
        assert_eq!(inches_to_mm(1.0), 25.4);
    }
}
