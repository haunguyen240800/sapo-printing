use crate::domain::printer::{Printer, PrinterStatus};

pub trait PrinterManager: Send + Sync {
    fn discover_printers(&self) -> Vec<Printer>;
    fn get_status(&self, name: &str) -> PrinterStatus;
}
