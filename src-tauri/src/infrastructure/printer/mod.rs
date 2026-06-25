pub mod printer_engine;
pub mod printer_manager;
pub mod printer_category;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(not(target_os = "windows"))]
pub mod cups;

pub use printer_engine::PrinterEngine;
pub use printer_manager::PrinterManager;
pub use printer_category::PrinterCategory;
