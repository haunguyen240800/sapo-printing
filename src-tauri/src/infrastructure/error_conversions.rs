//! Conversions from infrastructure-crate error types into `InfrastructureError`.
//!
//! These live in the infrastructure layer (not `shared`) so that `shared` stays
//! free of third-party infra crates (`reqwest`, `pdfium_render`) and of the
//! `infrastructure` module itself — keeping it at the bottom of the dependency
//! graph. The orphan rule is satisfied because `InfrastructureError` is a local
//! type, so its `From` impls may be defined anywhere in the crate.

use crate::application::errors::Error;
use crate::infrastructure::configs::db::DatabaseError;
use crate::infrastructure::errors::InfrastructureError;

impl From<reqwest::Error> for InfrastructureError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::TimeoutError(format!("Request timed out: {}", err))
        } else if err.is_connect() {
            Self::NetworkError(format!("Connection failed: {}", err))
        } else {
            Self::NetworkError(format!("Request failed: {}", err))
        }
    }
}

impl From<pdfium_render::prelude::PdfiumError> for InfrastructureError {
    fn from(err: pdfium_render::prelude::PdfiumError) -> Self {
        let msg = format!("{}", err);
        if msg.contains("format") || msg.contains("invalid") || msg.contains("corrupt") {
            Self::ValidationError(format!("Invalid PDF document: {}", msg))
        } else {
            Self::RenderError(format!("PDFium error: {}", msg))
        }
    }
}

impl From<DatabaseError> for InfrastructureError {
    fn from(err: DatabaseError) -> Self {
        Self::DatabaseError {
            reason: format!("{}", err),
        }
    }
}

/// Maps concrete infrastructure failures onto the semantic error contract that
/// application ports expose. This keeps `application` free of any dependency on
/// `InfrastructureError` while preserving a meaningful error classification.
impl From<InfrastructureError> for Error {
    fn from(err: InfrastructureError) -> Self {
        use InfrastructureError as I;
        match err {
            I::TimeoutError(msg) => Self::Timeout(msg),
            I::ValidationError(msg) => Self::InvalidInput(msg),
            I::CircuitOpenError => {
                Self::Unavailable("Circuit breaker is open — request rejected".to_string())
            }
            other => Self::Operation(other.to_string()),
        }
    }
}
