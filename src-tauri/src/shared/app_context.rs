use std::sync::Arc;

use crate::domain::print_job::PrintJobRepository;
use crate::domain::printer::PrinterRepository;
use crate::infrastructure::database::{
    run_migrations, DbPool, SqliteEventStore, SqlitePrintJobRepository, SqlitePrinterRepository,
};
use crate::infrastructure::printer::printer_manager::PrinterManager;
use crate::infrastructure::renderer::document_renderer::{DocumentRenderer, RenderConfig};
use crate::infrastructure::renderer::strategy_selector::StrategySelector;
use crate::infrastructure::secrets::SecretManager;
use crate::shared::errors::InfrastructureError;
use crate::shared::event_bus::EventBus;

// TODO (Story 3.1): Add DocumentDownloader to AppContext
// When Use Cases are created (Story 3.8+), inject:
//   use crate::infrastructure::downloader::{DocumentDownloader, ReqwestDownloader};
//   pub downloader: Arc<dyn DocumentDownloader>,
// Initialize in AppContext::new():
//   downloader: Arc::new(ReqwestDownloader::new()),

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
pub struct AppContext {
    pub job_repo: Arc<dyn PrintJobRepository>,
    pub printer_repo: Arc<dyn PrinterRepository>,
    pub printer_manager: Arc<dyn PrinterManager>,
    pub event_bus: Arc<dyn EventBus>,
    pub secret_manager: Arc<dyn SecretManager>,
    pub strategy_selector: Arc<StrategySelector>,
    pub event_store: Arc<SqliteEventStore>,
}

impl AppContext {
    /// Construct the application context for the given database path.
    ///
    /// # Errors
    /// Returns `InfrastructureError` if:
    /// - Linux: Secret Service daemon is not available
    /// - Any platform: Secret manager initialization fails
    /// - Database: Migration or pool creation fails
    #[allow(unused_variables)]
    pub fn new(db_path: &str) -> Result<Self, InfrastructureError> {
        let pool = DbPool::new(db_path)?;
        run_migrations(&mut pool.get()).map_err(|e| InfrastructureError::from(e))?;

        let job_repo: Arc<dyn PrintJobRepository> =
            Arc::new(SqlitePrintJobRepository::new(pool.get_arc()));
        let printer_repo: Arc<dyn PrinterRepository> =
            Arc::new(SqlitePrinterRepository::new(pool.get_arc()));
        let event_store: Arc<SqliteEventStore> = Arc::new(SqliteEventStore::new(pool.get_arc()));

        // Platform-specific secret manager initialization
        #[cfg(target_os = "windows")]
        let secret_manager: Arc<dyn SecretManager> = Arc::new(WindowsCredentialManager::new());

        #[cfg(target_os = "macos")]
        let secret_manager: Arc<dyn SecretManager> = Arc::new(MacOSKeychain::new());

        #[cfg(target_os = "linux")]
        let secret_manager: Arc<dyn SecretManager> = Arc::new(LinuxSecretService::new()?);

        // EventBus: in-memory for now (future: outbox pattern with persistent queue)
        let event_bus: Arc<dyn EventBus> =
            Arc::new(crate::shared::event_bus::InMemoryEventBus::new());

        // PrinterManager and StrategySelector — platform-specific, wired in later stories
        let printer_manager: Arc<dyn PrinterManager> = todo!("PrinterManager initialization");
        // The following is unreachable due to todo!() above — placeholder for when PrinterManager is wired in
        #[allow(unreachable_code)]
        let strategy_selector = Arc::new(StrategySelector::new(printer_manager.clone()));

        #[allow(unreachable_code)]
        Ok(Self {
            job_repo,
            printer_repo,
            printer_manager,
            event_bus,
            secret_manager,
            strategy_selector,
            event_store,
        })
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

    /// Selects the optimal renderer for the given printer and config.
    ///
    /// Delegates to `StrategySelector` which auto-detects printer capability
    /// and picks DirectPdfRenderer (fast path) or PdfiumRenderer (control path).
    pub fn renderer(&self, printer_name: &str, config: &RenderConfig) -> Arc<dyn DocumentRenderer> {
        self.strategy_selector.select_renderer(printer_name, config)
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
