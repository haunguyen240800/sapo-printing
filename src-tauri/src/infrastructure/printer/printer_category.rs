

/// Printer type classification for rendering strategy
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrinterCategory {
    /// PDF virtual printers (Microsoft Print to PDF, Adobe PDF, etc.)
    /// - No rendering needed, send raw PDF
    /// - Show save dialog
    PdfPrinter,

    /// Other virtual printers (XPS, Fax, OneNote)
    /// - May need rendering depending on driver
    VirtualPrinter,

    /// Physical/Network printers (HP, Epson, Canon, etc.)
    /// - Need bitmap rendering via PDFium
    PhysicalPrinter,
}

impl PrinterCategory {
    /// Detect printer category based on printer name
    pub fn detect(printer_name: &str) -> Self {
        let lower_name = printer_name.to_lowercase();

        // PDF printers - no rendering needed
        let pdf_patterns = [
            "microsoft print to pdf",
            "print to pdf",
            "save as pdf",
            "adobe pdf",
            "foxit reader pdf printer",
            "nitro pdf creator",
            "cutepdf writer",
            "dopdf",
            "pdf creator",
            "pdf printer",
        ];

        for pattern in &pdf_patterns {
            if lower_name.contains(pattern) {
                return Self::PdfPrinter;
            }
        }

        // Virtual printers - may need special handling
        let virtual_patterns = [
            "microsoft xps document writer",
            "microsoft print to image",
            "fax",
            "onenote",
            "send to onenote",
        ];

        for pattern in &virtual_patterns {
            if lower_name.contains(pattern) {
                return Self::VirtualPrinter;
            }
        }

        // Default: physical printer
        Self::PhysicalPrinter
    }

    /// Check if this printer needs bitmap rendering
    pub fn needs_rendering(&self) -> bool {
        match self {
            Self::PdfPrinter => false,
            Self::VirtualPrinter => false,
            Self::PhysicalPrinter => true,
        }
    }

    /// Check if this printer should show save dialog
    pub fn needs_save_dialog(&self) -> bool {
        matches!(self, Self::PdfPrinter)
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Self::PdfPrinter => "PDF Virtual Printer (raw PDF, no rendering)",
            Self::VirtualPrinter => "Virtual Printer (may need rendering)",
            Self::PhysicalPrinter => "Physical Printer (needs bitmap rendering)",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_pdf_printers() {
        assert_eq!(
            PrinterCategory::detect("Microsoft Print to PDF"),
            PrinterCategory::PdfPrinter
        );
        assert_eq!(
            PrinterCategory::detect("Adobe PDF"),
            PrinterCategory::PdfPrinter
        );
        assert_eq!(
            PrinterCategory::detect("CutePDF Writer"),
            PrinterCategory::PdfPrinter
        );
    }

    #[test]
    fn test_detect_virtual_printers() {
        assert_eq!(
            PrinterCategory::detect("Microsoft XPS Document Writer"),
            PrinterCategory::VirtualPrinter
        );
        assert_eq!(
            PrinterCategory::detect("Send to OneNote 2016"),
            PrinterCategory::VirtualPrinter
        );
    }

    #[test]
    fn test_detect_physical_printers() {
        assert_eq!(
            PrinterCategory::detect("HP LaserJet Pro"),
            PrinterCategory::PhysicalPrinter
        );
        assert_eq!(
            PrinterCategory::detect("Canon PIXMA"),
            PrinterCategory::PhysicalPrinter
        );
        assert_eq!(
            PrinterCategory::detect("Epson WorkForce"),
            PrinterCategory::PhysicalPrinter
        );
    }

    #[test]
    fn test_needs_rendering() {
        assert!(!PrinterCategory::PdfPrinter.needs_rendering());
        assert!(!PrinterCategory::VirtualPrinter.needs_rendering());
        assert!(PrinterCategory::PhysicalPrinter.needs_rendering());
    }

    #[test]
    fn test_needs_save_dialog() {
        assert!(PrinterCategory::PdfPrinter.needs_save_dialog());
        assert!(!PrinterCategory::VirtualPrinter.needs_save_dialog());
        assert!(!PrinterCategory::PhysicalPrinter.needs_save_dialog());
    }
}
