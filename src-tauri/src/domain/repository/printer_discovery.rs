use crate::domain::models::printer::Printer;

pub trait PrinterDiscovery: Send + Sync {
    fn list_printers(&self) -> Result<Vec<Printer>, String>;
}
