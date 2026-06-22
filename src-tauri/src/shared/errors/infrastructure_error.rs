#[derive(Debug)]
pub enum InfrastructureError {
    PrinterError { reason: String },
}

impl std::fmt::Display for InfrastructureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PrinterError { reason } => write!(f, "Printer error: {}", reason),
        }
    }
}

impl std::error::Error for InfrastructureError {}
