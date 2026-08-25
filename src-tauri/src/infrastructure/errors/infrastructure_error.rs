#[derive(Debug)]
pub enum InfrastructureError {
    PrinterError {
        reason: String,
    },
    /// HTTP/network failure (DNS error, connection refused, TLS error, etc.)
    NetworkError(String),
    /// Downloaded content failed validation (e.g. not a valid PDF)
    ValidationError(String),
    /// Operation exceeded the configured timeout
    TimeoutError(String),
    /// Circuit breaker is open — call rejected to prevent retry storm
    CircuitOpenError,
    /// PDFium-specific rendering failure (corrupt page, unsupported feature, etc.)
    RenderError(String),
    /// Database operation failed (SQLite error, migration failure, etc.)
    DatabaseError {
        reason: String,
    },
    /// Socket bind / listener setup failed (port in use, permission denied, etc.)
    BindError(String),
    /// Generic IO failure not covered by more specific variants
    IoError(String),
    /// Serialization / deserialization failed (JSON, etc.)
    SerializationError(String),
}

impl std::fmt::Display for InfrastructureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PrinterError { reason } => write!(f, "Printer error: {}", reason),
            Self::NetworkError(msg) => write!(f, "Network error: {}", msg),
            Self::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            Self::TimeoutError(msg) => write!(f, "Timeout error: {}", msg),
            Self::CircuitOpenError => {
                write!(f, "Circuit breaker is open — request rejected")
            }
            Self::RenderError(msg) => write!(f, "Render error: {}", msg),
            Self::DatabaseError { reason } => write!(f, "Database error: {}", reason),
            Self::BindError(msg) => write!(f, "Bind error: {}", msg),
            Self::IoError(msg) => write!(f, "IO error: {}", msg),
            Self::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
        }
    }
}

impl std::error::Error for InfrastructureError {}

impl From<std::io::Error> for InfrastructureError {
    fn from(err: std::io::Error) -> Self {
        use std::io::ErrorKind;
        match err.kind() {
            ErrorKind::TimedOut => Self::TimeoutError(format!("IO operation timed out: {}", err)),
            ErrorKind::ConnectionRefused
            | ErrorKind::ConnectionReset
            | ErrorKind::ConnectionAborted => {
                Self::NetworkError(format!("Connection error: {}", err))
            }
            ErrorKind::InvalidData => Self::ValidationError(format!("Invalid data: {}", err)),
            _ => Self::NetworkError(format!("IO error: {}", err)),
        }
    }
}
