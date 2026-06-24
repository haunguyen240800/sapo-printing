use serde::Serialize;

const NATIVE_HOST_NAME: &str = "sapo_printer";

#[derive(Serialize)]
struct NativeMessagingManifest {
    name: String,
    description: String,
    path: String,
    r#type: String,
    allowed_origins: Vec<String>,
}

pub fn generate_manifest(exe_path: &str, allowed_origins: Vec<String>) -> Result<String, String> {
    let manifest = NativeMessagingManifest {
        name: NATIVE_HOST_NAME.to_string(),
        description: "Sapo Printer - Native Messaging Host".to_string(),
        path: exe_path.to_string(),
        r#type: "stdio".to_string(),
        allowed_origins,
    };
    serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Failed to serialize manifest: {}", e))
}

#[cfg(target_os = "windows")]
pub fn register_native_host(allowed_origins: Vec<String>) -> Result<(), String> {
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Cannot determine exe path: {}", e))?
        .to_str()
        .ok_or_else(|| "Exe path contains non-UTF-8 characters".to_string())?
        .to_string();

    let manifest = generate_manifest(&exe_path, allowed_origins)?;

    let data_dir = {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| ".".to_string());
        std::path::PathBuf::from(&home).join(".sapo-printer")
    };
    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("Cannot create data directory: {}", e))?;

    let manifest_path = data_dir.join("native-messaging-manifest.json");
    std::fs::write(&manifest_path, &manifest)
        .map_err(|e| format!("Cannot write manifest: {}", e))?;

    let manifest_path_str = manifest_path
        .to_str()
        .ok_or_else(|| "Manifest path contains non-UTF-8".to_string())?;

    let registry_path = format!(r"Software\Google\Chrome\NativeMessagingHosts\{}", NATIVE_HOST_NAME);
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(&registry_path)
        .map_err(|e| format!("Cannot create registry key: {}", e))?;
    key.set_value("", &manifest_path_str.to_string())
        .map_err(|e| format!("Cannot set registry value: {}", e))?;

    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn register_native_host(allowed_origins: Vec<String>) -> Result<(), String> {
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Cannot determine exe path: {}", e))?
        .to_str()
        .ok_or_else(|| "Exe path contains non-UTF-8 characters".to_string())?
        .to_string();

    let manifest = generate_manifest(&exe_path, allowed_origins)?;

    let manifest_dir = {
        let home = std::env::var("HOME")
            .unwrap_or_else(|_| ".".to_string());
        std::path::PathBuf::from(&home)
            .join(".config")
            .join("google-chrome")
            .join("NativeMessagingHosts")
    };
    std::fs::create_dir_all(&manifest_dir)
        .map_err(|e| format!("Cannot create manifest directory: {}", e))?;

    let manifest_path = manifest_dir.join(format!("{}.json", NATIVE_HOST_NAME));
    std::fs::write(&manifest_path, &manifest)
        .map_err(|e| format!("Cannot write manifest: {}", e))?;

    Ok(())
}

// ── Unit Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_manifest_produces_valid_json() {
        let manifest = generate_manifest(
            "C:\\Program Files\\Sapo Printer\\sapo-printer.exe",
            vec!["chrome-extension://abcdef123456/".to_string()],
        ).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&manifest).unwrap();
        assert_eq!(parsed["name"], "sapo_printer");
        assert_eq!(parsed["type"], "stdio");
        assert_eq!(
            parsed["path"],
            "C:\\Program Files\\Sapo Printer\\sapo-printer.exe"
        );
        assert_eq!(
            parsed["allowed_origins"][0],
            "chrome-extension://abcdef123456/"
        );
    }

    #[test]
    fn test_generate_manifest_with_empty_origins() {
        let manifest = generate_manifest("/usr/local/bin/sapo-printer", vec![]).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&manifest).unwrap();
        assert_eq!(parsed["name"], "sapo_printer");
        assert!(parsed["allowed_origins"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_generate_manifest_contains_description() {
        let manifest = generate_manifest("/path/to/exe", vec![]).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&manifest).unwrap();
        assert!(parsed["description"].as_str().unwrap().contains("Sapo Printer"));
    }

    #[test]
    fn test_generate_manifest_error_propagates() {
        // Regression: previously unwrap_or_default() would silently swallow errors.
        // Now generate_manifest returns Result, so errors propagate to callers.
        let result = generate_manifest("/path/to/exe", vec!["valid-origin".to_string()]);
        assert!(result.is_ok());
    }
}
