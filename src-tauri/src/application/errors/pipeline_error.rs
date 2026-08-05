use std::fmt;

use crate::domain::print_job::PrintJobError;
use crate::shared::errors::InfrastructureError;

#[derive(Debug)]
pub enum PipelineError {
    Domain(PrintJobError),
    Infrastructure(InfrastructureError),
    Persistence(String),
}

impl PipelineError {
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Infrastructure(e) => crate::application::policies::retry_policy::is_retryable(e),
            Self::Domain(_) | Self::Persistence(_) => false,
        }
    }
}

impl fmt::Display for PipelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Domain(e) => write!(f, "Domain error: {}", e),
            Self::Infrastructure(e) => write!(f, "{}", e),
            Self::Persistence(msg) => write!(f, "Persistence error: {}", msg),
        }
    }
}

impl std::error::Error for PipelineError {}

impl From<PrintJobError> for PipelineError {
    fn from(e: PrintJobError) -> Self {
        Self::Domain(e)
    }
}

impl From<InfrastructureError> for PipelineError {
    fn from(e: InfrastructureError) -> Self {
        Self::Infrastructure(e)
    }
}
