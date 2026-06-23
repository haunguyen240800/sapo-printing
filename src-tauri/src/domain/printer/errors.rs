use std::fmt;

/// Domain errors for the Printer aggregate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrinterDomainError {
    /// Attempted to assign a print job to a printer that is not online.
    PrinterNotOnline { printer_name: String },
    /// Repository operation failed (persistence, query, etc.).
    RepositoryError { reason: String },
}

impl fmt::Display for PrinterDomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PrinterDomainError::PrinterNotOnline { printer_name } => {
                write!(f, "Printer '{}' is not online", printer_name)
            }
            PrinterDomainError::RepositoryError { reason } => {
                write!(f, "Repository error: {}", reason)
            }
        }
    }
}

impl std::error::Error for PrinterDomainError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_printer_not_online_display() {
        let err = PrinterDomainError::PrinterNotOnline {
            printer_name: "HP LaserJet".to_string(),
        };
        assert!(err.to_string().contains("HP LaserJet"));
    }

    #[test]
    fn test_printer_not_online_eq() {
        let a = PrinterDomainError::PrinterNotOnline { printer_name: "X".to_string() };
        let b = PrinterDomainError::PrinterNotOnline { printer_name: "X".to_string() };
        assert_eq!(a, b);
    }
}
