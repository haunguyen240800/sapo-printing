#[cfg(target_os = "windows")]
use crate::shared::errors::InfrastructureError;
#[cfg(target_os = "windows")]
use super::super::printer_engine::PrinterEngine;

pub struct WindowsPrinterEngine;

impl WindowsPrinterEngine {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsPrinterEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "windows")]
impl PrinterEngine for WindowsPrinterEngine {
    fn print(&self, _printer_name: &str, _data: &[u8]) -> Result<(), InfrastructureError> {
        Ok(())
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    #[test]
    fn test_print_stub_returns_ok() {
        let engine = WindowsPrinterEngine::new();
        assert!(engine.print("any_printer", &[]).is_ok());
    }
}
