#[cfg(not(target_os = "windows"))]
pub mod cups_printer_engine;
#[cfg(not(target_os = "windows"))]
pub mod cups_printer_manager;

#[cfg(not(target_os = "windows"))]
pub use cups_printer_engine::CupsPrinterEngine;
#[cfg(not(target_os = "windows"))]
pub use cups_printer_manager::CupsPrinterManager;
