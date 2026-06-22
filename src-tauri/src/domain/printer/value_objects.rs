use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

/// Stable unique identifier for a printer, backed by UUID v4.
/// Distinct from `PrinterName` — survives printer renames.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrinterId(Uuid);

impl PrinterId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for PrinterId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PrinterId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for PrinterId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

/// Human-readable printer name. Used by `PrintJob` to reference a printer.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrinterName(String);

impl PrinterName {
    pub fn new(name: String) -> Self {
        Self(name)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PrinterName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Printer availability states.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrinterStatus {
    Online,
    Offline,
    Error,
}

/// Physical or logical printer connection type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrinterType {
    Local,
    Network,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_printer_id_unique() {
        let id1 = PrinterId::new();
        let id2 = PrinterId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_printer_id_display_is_uuid_format() {
        let id = PrinterId::new();
        let s = id.to_string();
        assert_eq!(s.len(), 36);
        assert!(s.contains('-'));
    }

    #[test]
    fn test_printer_id_from_str_roundtrip() {
        let id = PrinterId::new();
        let parsed: PrinterId = id.to_string().parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_printer_name_as_str() {
        let name = PrinterName::new("HP LaserJet".to_string());
        assert_eq!(name.as_str(), "HP LaserJet");
    }

    #[test]
    fn test_printer_name_display() {
        let name = PrinterName::new("Canon".to_string());
        assert_eq!(name.to_string(), "Canon");
    }

    #[test]
    fn test_printer_status_eq() {
        assert_eq!(PrinterStatus::Online, PrinterStatus::Online);
        assert_ne!(PrinterStatus::Online, PrinterStatus::Offline);
    }

    #[test]
    fn test_printer_type_eq() {
        assert_eq!(PrinterType::Local, PrinterType::Local);
        assert_ne!(PrinterType::Local, PrinterType::Network);
    }
}
