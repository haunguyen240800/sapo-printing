use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a document, backed by UUID v4.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentId(Uuid);

impl DocumentId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for DocumentId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for DocumentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Supported document types.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentType {
    Pdf,
}

/// Location (URL) of a document resource.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentLocation(String);

impl DocumentLocation {
    pub fn new(s: String) -> Self {
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DocumentLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_id_unique() {
        let id1 = DocumentId::new();
        let id2 = DocumentId::new();
        assert_ne!(id1, id2);
    }
}
