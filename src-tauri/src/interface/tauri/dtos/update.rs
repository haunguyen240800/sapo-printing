use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCheckResponse {
    pub update_available: bool,
    pub version: Option<String>,
    pub release_notes: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::updater::update_checker::UpdateCheckResult;

    #[test]
    fn test_dto_serialize() {
        let dto = UpdateCheckResponse {
            update_available: true,
            version: Some("0.2.0".to_string()),
            release_notes: Some("Fixes".to_string()),
        };
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json["update_available"], true);
        assert_eq!(json["version"], "0.2.0");
        assert_eq!(json["release_notes"], "Fixes");
    }

    #[test]
    fn test_dto_deserialize() {
        let json = r#"{"update_available":false,"version":null,"release_notes":null}"#;
        let dto: UpdateCheckResponse = serde_json::from_str(json).unwrap();
        assert!(!dto.update_available);
        assert!(dto.version.is_none());
        assert!(dto.release_notes.is_none());
    }

    #[test]
    fn test_infrastructure_result_to_dto_mapping() {
        let result = UpdateCheckResult {
            update_available: true,
            version: Some("1.0.0".to_string()),
            release_notes: Some("New features".to_string()),
        };
        let dto = UpdateCheckResponse {
            update_available: result.update_available,
            version: result.version.clone(),
            release_notes: result.release_notes.clone(),
        };
        assert_eq!(dto.update_available, result.update_available);
        assert_eq!(dto.version, result.version);
        assert_eq!(dto.release_notes, result.release_notes);
    }

    #[test]
    fn test_dto_fields_match_infrastructure_result() {
        let result = UpdateCheckResult {
            update_available: false,
            version: None,
            release_notes: None,
        };
        let dto = UpdateCheckResponse {
            update_available: result.update_available,
            version: result.version.clone(),
            release_notes: result.release_notes.clone(),
        };
        assert_eq!(dto.update_available, false);
        assert!(dto.version.is_none());
        assert!(dto.release_notes.is_none());
    }
}
