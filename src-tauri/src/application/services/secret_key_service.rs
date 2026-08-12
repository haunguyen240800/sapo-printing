use crate::application::errors::Error;

pub const MAX_SECRET_SIZE: usize = 2560;

pub fn format_key(key: &str) -> String {
    if key.contains('/') {
        panic!("Key cannot contain '/' character: {}", key);
    }
    format!("com.sapo.printer/{}", key)
}

pub fn validate_key(key: &str) -> Result<(), Error> {
    if key.is_empty() {
        return Err(Error::InvalidInput("Key cannot be empty".to_string()));
    }
    if key.contains('/') {
        return Err(Error::InvalidInput(format!(
            "Key cannot contain '/' character: {}",
            key
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_key_adds_namespace() {
        assert_eq!(format_key("device_token"), "com.sapo.printer/device_token");
        assert_eq!(format_key("hmac_key"), "com.sapo.printer/hmac_key");
    }

    #[test]
    fn test_format_key_handles_empty_string() {
        assert_eq!(format_key(""), "com.sapo.printer/");
    }

    #[test]
    fn test_format_key_handles_special_chars() {
        assert_eq!(format_key("user:token"), "com.sapo.printer/user:token");
    }

    #[test]
    fn test_validate_key_rejects_empty() {
        assert!(validate_key("").is_err());
    }

    #[test]
    fn test_validate_key_rejects_slash() {
        assert!(validate_key("a/b").is_err());
    }

    #[test]
    fn test_validate_key_accepts_normal() {
        assert!(validate_key("device_token").is_ok());
    }
}
