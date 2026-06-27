use std::fmt;

/// Domain errors for the Document aggregate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocumentDomainError {
    /// URL failed validation (empty or unsupported scheme).
    InvalidUrl { url: String },
}

impl fmt::Display for DocumentDomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentDomainError::InvalidUrl { url } => {
                write!(f, "Invalid document URL: '{}'", url)
            }
        }
    }
}

impl std::error::Error for DocumentDomainError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_url_display_contains_url() {
        let err = DocumentDomainError::InvalidUrl {
            url: "ftp://bad.example".to_string(),
        };
        assert!(err.to_string().contains("ftp://bad.example"));
    }
}
