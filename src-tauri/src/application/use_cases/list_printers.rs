use std::sync::Arc;
use crate::domain::models::printer::Printer;
use crate::domain::repository::printer_discovery::PrinterDiscovery;
use crate::application::use_cases::errors::ApplicationError;

pub struct ListPrintersUseCase {
    discovery: Arc<dyn PrinterDiscovery>,
}

impl ListPrintersUseCase {
    pub fn new(discovery: Arc<dyn PrinterDiscovery>) -> Self {
        Self { discovery }
    }

    pub fn execute(&self) -> Result<Vec<Printer>, ApplicationError> {
        self.discovery
            .list_printers()
            .map_err(|e| ApplicationError::RepositoryError(format!("Failed to list printers: {}", e)))
    }
}
