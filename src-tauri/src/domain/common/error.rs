use std::fmt;

#[derive(Debug, Clone)]
pub struct DomainValidationException {
    pub message: String,
}

impl DomainValidationException {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for DomainValidationException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Domain Validation Error: {}", self.message)
    }
}

impl std::error::Error for DomainValidationException {}
