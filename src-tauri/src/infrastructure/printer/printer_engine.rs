use crate::shared::errors::InfrastructureError;

pub trait PrinterEngine: Send + Sync {
    fn print(&self, printer_name: &str, data: &[u8]) -> Result<(), InfrastructureError>;
}
