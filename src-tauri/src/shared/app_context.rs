use std::sync::Arc;

use crate::application::ports::{EventStore, SecretManager};
use crate::domain::print_job::PrintJobRepository;
use crate::infrastructure::configs::db::{run_migrations, DbPool};
use crate::infrastructure::persistence::sqlite::{SqliteEventStore, SqlitePrintJobRepository};


use crate::shared::errors::InfrastructureError;
use crate::shared::event_bus::EventBus;

// TODO (Story 3.1): Add DocumentDownloadService to AppContext
// When Use Cases are created (Story 3.8+), inject:
//   use crate::application::ports::DocumentDownloadService;
//   use crate::infrastructure::integrations::network::ReqwestDownloader;
//   pub downloader: Arc<dyn DocumentDownloadService>,
// Initialize in AppContext::new():
//   downloader: Arc::new(ReqwestDownloader::new()),

#[cfg(target_os = "linux")]
use crate::infrastructure::platform::keychain::LinuxSecretService;
#[cfg(target_os = "macos")]
use crate::infrastructure::platform::keychain::MacOSKeychain;
#[cfg(target_os = "windows")]
use crate::infrastructure::platform::keychain::WindowsCredentialManager;

/// Application-wide dependency injection container.
///
/// Holds `Arc`-wrapped trait objects for all cross-cutting dependencies.
/// Constructed once in `main.rs` and shared across use cases.
pub struct AppContext {
    pub job_repo: Arc<dyn PrintJobRepository>,
    pub event_bus: Arc<dyn EventBus>,
    pub secret_manager: Arc<dyn SecretManager>,
    pub event_store: Arc<dyn EventStore>,
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
        {
            let mut conn = pool.get().map_err(InfrastructureError::from)?;
            run_migrations(&mut *conn).map_err(InfrastructureError::from)?;
        }

        let job_repo: Arc<dyn PrintJobRepository> =
            Arc::new(SqlitePrintJobRepository::new(pool.clone()));

        // Platform-specific secret manager initialization
        #[cfg(target_os = "windows")]
        let secret_manager: Arc<dyn SecretManager> = Arc::new(WindowsCredentialManager::new());

        #[cfg(target_os = "macos")]
        let secret_manager: Arc<dyn SecretManager> = Arc::new(MacOSKeychain::new());

        #[cfg(target_os = "linux")]
        let secret_manager: Arc<dyn SecretManager> = Arc::new(LinuxSecretService::new()?);

        let event_store: Arc<dyn EventStore> = Arc::new(SqliteEventStore::new(pool.clone(), secret_manager.clone()));

        // EventBus: in-memory for now (future: outbox pattern with persistent queue)
        let event_bus: Arc<dyn EventBus> =
            Arc::new(crate::shared::event_bus::InMemoryEventBus::new());

        // PrinterManager and StrategySelector — platform-specific, wired in later stories
        // The following is unreachable due to todo!() above — placeholder for when PrinterManager is wired in
        #[allow(unreachable_code)]

        #[allow(unreachable_code)]
        Ok(Self {
            job_repo,

            event_bus,
            secret_manager,
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
