use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

/// Unique identifier for a print job, backed by UUID v4.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(Uuid);

impl JobId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for JobId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for JobId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_id_unique() {
        let id1 = JobId::new();
        let id2 = JobId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_job_id_to_string() {
        let id = JobId::new();
        let s = id.to_string();
        assert_eq!(s.len(), 36); // UUID format: 8-4-4-4-12
        assert!(s.contains('-'));
    }

    #[test]
    fn test_job_id_from_str() {
        let id = JobId::new();
        let s = id.to_string();
        let parsed: JobId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn test_job_id_from_str_invalid() {
        let result: Result<JobId, _> = "not-a-uuid".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_job_id_clone_eq() {
        let id = JobId::new();
        let cloned = id.clone();
        assert_eq!(id, cloned);
    }
}
