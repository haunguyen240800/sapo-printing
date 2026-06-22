use std::{env, fs, io, path::PathBuf};

use serde_json::Value;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=tauri.conf.json");

    let config_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?).join("tauri.conf.json");
    let config_text = fs::read_to_string(&config_path).map_err(|error| {
        invalid_config(format!("cannot read {}: {error}", config_path.display()))
    })?;
    let config: Value = serde_json::from_str(&config_text)
        .map_err(|error| invalid_config(format!("tauri.conf.json is not valid JSON: {error}")))?;

    let product_name = required_string(&config, &["productName"])?;
    emit_env("SAPO_PRODUCT_NAME", product_name)?;
    emit_env("SAPO_APP_SLUG", &slugify(product_name))?;
    emit_env("SAPO_APP_VERSION", required_string(&config, &["version"])?)?;
    emit_env(
        "SAPO_UPDATER_PUBKEY",
        required_string(&config, &["plugins", "updater", "pubkey"])?,
    )?;
    emit_env("SAPO_UPDATER_ENDPOINT", required_updater_endpoint(&config)?)?;

    tauri_build::build();
    Ok(())
}

fn required_string<'a>(config: &'a Value, path: &[&str]) -> Result<&'a str, io::Error> {
    let display_path = path.join(".");
    let mut value = config;
    for segment in path {
        value = value
            .get(*segment)
            .ok_or_else(|| invalid_config(format!("missing required field `{display_path}`")))?;
    }

    let value = value.as_str().ok_or_else(|| {
        invalid_config(format!("required field `{display_path}` must be a string"))
    })?;
    validate_env_value(&display_path, value)?;
    Ok(value)
}

fn required_updater_endpoint(config: &Value) -> Result<&str, io::Error> {
    let path = "plugins.updater.endpoints";
    let endpoints = config
        .pointer("/plugins/updater/endpoints")
        .ok_or_else(|| invalid_config(format!("missing required field `{path}`")))?
        .as_array()
        .ok_or_else(|| invalid_config(format!("required field `{path}` must be an array")))?;
    if endpoints.len() != 1 {
        return Err(invalid_config(format!(
            "required field `{path}` must contain exactly one fixed endpoint; found {}",
            endpoints.len()
        )));
    }
    let endpoint = endpoints
        .first()
        .ok_or_else(|| invalid_config(format!("required field `{path}` must not be empty")))?
        .as_str()
        .ok_or_else(|| invalid_config(format!("`{path}[0]` must be a string")))?;
    validate_env_value("plugins.updater.endpoints[0]", endpoint)?;
    if endpoint.contains("{{") || endpoint.contains("}}") {
        return Err(invalid_config(format!(
            "`{path}[0]` must be a fixed URL without Tauri template variables"
        )));
    }
    Ok(endpoint)
}

fn validate_env_value(path: &str, value: &str) -> Result<(), io::Error> {
    if value.trim().is_empty() {
        return Err(invalid_config(format!(
            "required field `{path}` must not be empty"
        )));
    }
    if value.contains(['\r', '\n']) {
        return Err(invalid_config(format!(
            "required field `{path}` must be a single-line value"
        )));
    }
    Ok(())
}

fn slugify(value: &str) -> String {
    let mut slug = String::with_capacity(value.len());
    let mut prev_hyphen = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            prev_hyphen = false;
        } else if !slug.is_empty() && !prev_hyphen {
            slug.push('-');
            prev_hyphen = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
}

fn emit_env(name: &str, value: &str) -> Result<(), io::Error> {
    validate_env_value(name, value)?;
    println!("cargo:rustc-env={name}={value}");
    Ok(())
}

fn invalid_config(message: String) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("invalid tauri.conf.json: {message}"),
    )
}
