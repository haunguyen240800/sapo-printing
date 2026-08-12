use crate::application::dto::PrinterDto;
use crate::application::errors::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrinterAvailability {
    Online,
    Offline,
    Unknown,
}

pub trait PrinterPort: Send + Sync {
    fn list(&self) -> Result<Vec<PrinterDto>, Error>;

    fn availability(&self, printer_id: &str) -> Result<PrinterAvailability, Error>;
}
