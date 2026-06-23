#[cfg(not(target_os = "windows"))]
use super::super::printer_engine::PrinterEngine;
#[cfg(not(target_os = "windows"))]
use crate::shared::errors::InfrastructureError;

#[cfg(not(target_os = "windows"))]
pub struct CupsPrinterEngine;

#[cfg(not(target_os = "windows"))]
impl CupsPrinterEngine {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(target_os = "windows"))]
impl Default for CupsPrinterEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(target_os = "windows"))]
impl PrinterEngine for CupsPrinterEngine {
    fn print(&self, _printer_name: &str, _data: &[u8]) -> Result<(), InfrastructureError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_stub_returns_ok() {
        let engine = CupsPrinterEngine::new();
        assert!(engine.print("any_printer", &[]).is_ok());
    }
}
