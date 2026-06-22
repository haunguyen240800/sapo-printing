use std::sync::Arc;

use crate::application::errors::Error;
use crate::application::models::PrinterResponse;
use crate::application::ports::PrinterPort;

pub struct ListPrintersUseCase {
    printer_manager: Arc<dyn PrinterPort>,
}

impl ListPrintersUseCase {
    pub fn new(printer_manager: Arc<dyn PrinterPort>) -> Self {
        Self { printer_manager }
    }

    pub fn execute(&self) -> Result<Vec<PrinterResponse>, Error> {
        self.printer_manager
            .list()
            .map_err(|e| Error::RepositoryError(format!("Failed to list printers: {}", e)))
    }
}
