use crate::application::dto::PrinterDto;
use crate::shared::errors::InfrastructureError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrinterAvailability {
    Online,
    Offline,
    Unknown,
}

pub trait PrinterManager: Send + Sync {
    fn list(&self) -> Result<Vec<PrinterDto>, InfrastructureError>;

    fn availability(&self, printer_id: &str) -> Result<PrinterAvailability, InfrastructureError>;
}
