//! `PrinterDto` — application-layer query DTO for printer information.
//!
//! Carries OS-reported printer metadata across the application boundary.
//! The printer itself is **not** a domain aggregate in this application
//! (the OS owns the resource), so this DTO replaces what would otherwise
//! have been a domain `Printer` struct.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterDto {
    /// OS device identifier (Windows queue name, CUPS queue name).
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Current OS-reported status: "Online" | "Offline" | "Error" | "Unknown".
    pub status: String,
    /// Connection type: "Local" | "Network" | "Virtual" | "PDF" | "Unknown".
    pub printer_type: String,
    /// Whether this is the OS default printer.
    pub is_default: bool,
}
