//! Factory: build use cases from shared dependencies.
//!
//! Interface layer holds `Arc<UseCaseFactory>` instead of raw repo/port refs.
//! Keeps Application internals out of Interface (Clean Arch — Interface calls
//! Use Cases only, does not know their construction).

use std::sync::Arc;

use crate::application::ports::{ConfigProvider, EventStore, PrinterManager};
use crate::application::use_cases::{
    CreatePrintJobUseCase, GetJobStatusUseCase, ListPrintersUseCase,
};
use crate::domain::print_job::PrintJobRepository;
use crate::shared::event_bus::EventBus;

pub struct UseCaseFactory {
    job_repo: Arc<dyn PrintJobRepository>,
    event_store: Arc<dyn EventStore>,
    event_bus: Arc<dyn EventBus>,
    config_provider: Arc<dyn ConfigProvider>,
    printer_manager: Arc<dyn PrinterManager>,
}

impl UseCaseFactory {
    pub fn new(
        job_repo: Arc<dyn PrintJobRepository>,
        event_store: Arc<dyn EventStore>,
        event_bus: Arc<dyn EventBus>,
        config_provider: Arc<dyn ConfigProvider>,
        printer_manager: Arc<dyn PrinterManager>,
    ) -> Self {
        Self {
            job_repo,
            event_store,
            event_bus,
            config_provider,
            printer_manager,
        }
    }

    pub fn create_print_job(&self) -> CreatePrintJobUseCase {
        CreatePrintJobUseCase {
            job_repo: self.job_repo.clone(),
            event_store: self.event_store.clone(),
            event_bus: self.event_bus.clone(),
            config_provider: self.config_provider.clone(),
            printer_manager: self.printer_manager.clone(),
        }
    }

    pub fn get_job_status(&self) -> GetJobStatusUseCase {
        GetJobStatusUseCase::new(self.job_repo.clone())
    }

    pub fn list_printers(&self) -> ListPrintersUseCase {
        ListPrintersUseCase::new(self.printer_manager.clone())
    }
}
