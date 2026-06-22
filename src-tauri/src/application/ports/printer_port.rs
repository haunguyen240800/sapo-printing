use crate::application::errors::Error;
use crate::application::models::PrinterResponse;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrinterAvailability {
    Online,
    Offline,
    Unknown,
}

pub trait PrinterPort: Send + Sync {
    fn list(&self) -> Result<Vec<PrinterResponse>, Error>;

    fn availability(&self, printer_id: &str) -> Result<PrinterAvailability, Error>;
}
