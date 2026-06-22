use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

/// Unique identifier for a print job, backed by UUID v4.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrintJobId(Uuid);

impl PrintJobId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for PrintJobId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PrintJobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for PrintJobId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_job_id_unique() {
        let id1 = PrintJobId::new();
        let id2 = PrintJobId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_print_job_id_to_string() {
        let id = PrintJobId::new();
        let s = id.to_string();
        assert_eq!(s.len(), 36);
        assert!(s.contains('-'));
    }

    #[test]
    fn test_print_job_id_from_str() {
        let id = PrintJobId::new();
        let s = id.to_string();
        let parsed: PrintJobId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_print_job_id_from_str_invalid() {
        let result: Result<PrintJobId, _> = "not-a-uuid".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_print_job_id_clone_eq() {
        let id = PrintJobId::new();
        let cloned = id.clone();
        assert_eq!(id, cloned);
    }
}
