use std::sync::Arc;

use crate::application::dto::PrinterDto;
use crate::application::errors::ApplicationError;
use crate::application::ports::PrinterManager;

pub struct ListPrintersUseCase {
    printer_manager: Arc<dyn PrinterManager>,
}

impl ListPrintersUseCase {
    pub fn new(printer_manager: Arc<dyn PrinterManager>) -> Self {
        Self { printer_manager }
    }

    pub fn execute(&self) -> Result<Vec<PrinterDto>, ApplicationError> {
        self.printer_manager.list().map_err(|e| {
            ApplicationError::RepositoryError(format!("Failed to list printers: {}", e))
        })
    }
}
