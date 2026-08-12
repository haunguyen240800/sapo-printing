use std::path::PathBuf;
use std::sync::OnceLock;

use pdfium_render::prelude::Pdfium;

use crate::infrastructure::errors::InfrastructureError;

static PDFIUM_RESOURCE_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn set_pdfium_resource_dir(dir: PathBuf) {
    let _ = PDFIUM_RESOURCE_DIR.set(dir);
}

pub fn load_pdfium() -> Result<Pdfium, InfrastructureError> {
    let mut last_error: Option<String> = None;

    if let Some(dir) = PDFIUM_RESOURCE_DIR.get() {
        let dir_str = dir.to_string_lossy().to_string();
        match Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(&dir_str)) {
            Ok(bind) => return Ok(Pdfium::new(bind)),
            Err(e) => last_error = Some(format!("resource_dir {:?}: {:?}", dir, e)),
        }
    }

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
