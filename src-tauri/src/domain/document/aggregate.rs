use serde::{Deserialize, Serialize};

use super::errors::DocumentDomainError;
use super::value_objects::{DocumentId, DocumentLocation, DocumentType};

/// Document aggregate root.
///
/// Represents a printable document identified by a URL. Validates that the
/// location is a reachable HTTP/HTTPS resource before creation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    id: DocumentId,
    doc_type: DocumentType,
    location: DocumentLocation,
}

impl Document {
    /// Creates a new Document, validating the URL scheme.
    ///
    /// # Errors
    /// Returns `InvalidUrl` if the location is empty or does not start with
    /// `http://` or `https://`.
    pub fn new(
        location: DocumentLocation,
        doc_type: DocumentType,
    ) -> Result<Self, DocumentDomainError> {
        let url = location.as_str();
        if url.is_empty() || (!url.starts_with("http://") && !url.starts_with("https://")) {
            return Err(DocumentDomainError::InvalidUrl {
                url: url.to_string(),
            });
        }
        Ok(Self {
            id: DocumentId::new(),
            doc_type,
            location,
        })
    }

    pub fn id(&self) -> &DocumentId {
        &self.id
    }

    pub fn doc_type(&self) -> &DocumentType {
        &self.doc_type
    }

    pub fn location(&self) -> &DocumentLocation {
        &self.location
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_new_valid_https() {
        let loc = DocumentLocation::new("https://s3.example.com/doc.pdf".to_string());
        let result = Document::new(loc, DocumentType::Pdf);
        assert!(result.is_ok());
    }

    #[test]
    fn test_document_new_valid_http() {
        let loc = DocumentLocation::new("http://example.com/doc.pdf".to_string());
        let result = Document::new(loc, DocumentType::Pdf);
        assert!(result.is_ok());
    }

    #[test]
    fn test_document_new_empty_url() {
        let loc = DocumentLocation::new(String::new());
        let result = Document::new(loc, DocumentType::Pdf);
        assert!(matches!(
            result,
            Err(DocumentDomainError::InvalidUrl { .. })
        ));
    }

    #[test]
    fn test_document_new_invalid_scheme() {
        let loc = DocumentLocation::new("ftp://example.com/doc.pdf".to_string());
        let result = Document::new(loc, DocumentType::Pdf);
        assert!(matches!(
            result,
            Err(DocumentDomainError::InvalidUrl { .. })
        ));
    }
}
