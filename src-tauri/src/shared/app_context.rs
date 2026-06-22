use std::sync::Arc;

use crate::domain::print_job::PrintJobRepository;
use crate::domain::printer::PrinterRepository;
use crate::shared::event_bus::EventBus;

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
    pub event_bus: Arc<dyn EventBus>,
}

impl AppContext {
    /// Construct the application context for the given database path.
    ///
    /// # Panics
    /// Panics with `todo!()` until Epic 2 wires in real infrastructure
    /// implementations (`SqlitePrintJobRepository`, etc.).
    pub fn new(_db_path: &str) -> Self {
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
