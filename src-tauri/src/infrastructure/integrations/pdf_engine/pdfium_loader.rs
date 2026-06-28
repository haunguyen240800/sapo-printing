//! Shared PDFium binding loader for all `RenderStrategy` implementations.
//!
//! The resource directory (where the PDFium shared library is bundled) is
//! injected once at application startup via [`set_pdfium_resource_dir`].
//! At runtime the loader tries (in order):
//!   1. The configured resource directory (Tauri `app.path().resource_dir()`)
//!   2. `./bin/` and current directory (developer / unit-test fallback)
//!   3. The system library path
//!
//! Falling back to CWD lets `cargo test` work without the user pre-installing
//! PDFium globally, while the resource directory keeps installed builds
//! independent of the working directory the app happens to be launched from.

use std::path::PathBuf;
use std::sync::OnceLock;

use pdfium_render::prelude::Pdfium;

use crate::shared::errors::InfrastructureError;

/// Resource directory configured at startup (e.g. Tauri resource dir).
/// Set once; subsequent calls are no-ops.
static PDFIUM_RESOURCE_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Register the directory containing the bundled PDFium library. Idempotent:
/// only the first call wins so unit tests can leave it unset.
pub fn set_pdfium_resource_dir(dir: PathBuf) {
    let _ = PDFIUM_RESOURCE_DIR.set(dir);
}

/// Load PDFium with the project's fallback search order.
pub fn load_pdfium() -> Result<Pdfium, InfrastructureError> {
    let mut last_error: Option<String> = None;

    // 1. Try the registered resource directory first.
    if let Some(dir) = PDFIUM_RESOURCE_DIR.get() {
        let dir_str = dir.to_string_lossy().to_string();
        match Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&dir_str)) {
            Ok(bind) => return Ok(Pdfium::new(bind)),
            Err(e) => last_error = Some(format!("resource_dir {:?}: {:?}", dir, e)),
        }
    }

    // 2. Developer / unit-test fallbacks: ./bin/ then CWD.
    let bind = Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./bin/"))
        .or_else(|_| Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path("./")))
        .or_else(|_| Pdfium::bind_to_system_library())
        .map_err(|e| {
            InfrastructureError::RenderError(format!(
                "Failed to load PDFium library (last_error={:?}, fallback={:?})",
                last_error, e
            ))
        })?;
    Ok(Pdfium::new(bind))
}
