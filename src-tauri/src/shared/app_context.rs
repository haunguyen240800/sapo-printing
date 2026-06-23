use std::sync::Arc;

use crate::domain::print_job::PrintJobRepository;
use crate::domain::printer::PrinterRepository;
use crate::infrastructure::printer::printer_manager::PrinterManager;
use crate::infrastructure::secrets::SecretManager;
use crate::shared::errors::InfrastructureError;
use crate::shared::event_bus::EventBus;

// TODO (Story 3.1): Add DocumentDownloader to AppContext
// When Use Cases are created (Story 3.8+), inject:
//   use crate::infrastructure::downloader::{DocumentDownloader, ReqwestDownloader};
//   pub downloader: Arc<dyn DocumentDownloader>,
// Initialize in AppContext::new():
//   downloader: Arc::new(ReqwestDownloader::new()),

// TODO (Story 3.2): Add DocumentRenderer to AppContext
// When Use Cases are created (Story 3.8+), inject:
//   use crate::infrastructure::renderer::{DocumentRenderer, PdfiumRenderer};
//   pub renderer: Arc<dyn DocumentRenderer>,
// Initialize in AppContext::new():
//   renderer: Arc::new(PdfiumRenderer::new(300)),

#[cfg(target_os = "linux")]
use crate::infrastructure::secrets::LinuxSecretService;
#[cfg(target_os = "macos")]
use crate::infrastructure::secrets::MacOSKeychain;
#[cfg(target_os = "windows")]
use crate::infrastructure::secrets::WindowsCredentialManager;

/// Application-wide dependency injection container.
///
/// Holds `Arc`-wrapped trait objects for all cross-cutting dependencies.
/// Constructed once in `main.rs` and shared across use cases.
///
/// Infrastructure implementations (repositories, event bus) are wired in
/// Epic 2. Until then, `AppContext::new()` panics with `todo!()`.
pub struct AppContext {
    pub job_repo: Arc<dyn PrintJobRepository>,
    pub printer_repo: Arc<dyn PrinterRepository>,
    pub printer_manager: Arc<dyn PrinterManager>,
    pub event_bus: Arc<dyn EventBus>,
    pub secret_manager: Arc<dyn SecretManager>,
}

impl AppContext {
    /// Construct the application context for the given database path.
    ///
    /// # Errors
    /// Returns `InfrastructureError` if:
    /// - Linux: Secret Service daemon is not available
    /// - Any platform: Secret manager initialization fails
    ///
    /// # Panics
    /// Panics with `todo!()` until Epic 2 wires in real infrastructure
    /// implementations (`SqlitePrintJobRepository`, etc.).
    pub fn new(_db_path: &str) -> Result<Self, InfrastructureError> {
        // Platform-specific secret manager initialization
        #[cfg(target_os = "windows")]
        let secret_manager: Arc<dyn SecretManager> = Arc::new(WindowsCredentialManager::new());

        #[cfg(target_os = "macos")]
        let secret_manager: Arc<dyn SecretManager> = Arc::new(MacOSKeychain::new());

        #[cfg(target_os = "linux")]
        let secret_manager: Arc<dyn SecretManager> = Arc::new(LinuxSecretService::new()?);

        todo!("AppContext::new — infrastructure not yet implemented (Epic 2, Stories 2.1/2.4)")
    }

    /// Returns the name of the platform-specific printer engine that will be
    /// used by this instance.
    pub fn platform_engine_name() -> &'static str {
        #[cfg(target_os = "windows")]
        {
            "windows"
        }
        #[cfg(not(target_os = "windows"))]
        {
            "cups"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_engine_name_nonempty() {
        let name = AppContext::platform_engine_name();
        assert!(!name.is_empty());
    }
}
