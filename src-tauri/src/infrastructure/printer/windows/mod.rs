#[cfg(target_os = "windows")]
pub mod win32_printer_manager;
#[cfg(target_os = "windows")]
pub mod windows_printer_engine;

#[cfg(target_os = "windows")]
pub use win32_printer_manager::Win32PrinterManager;
#[cfg(target_os = "windows")]
pub use windows_printer_engine::WindowsPrinterEngine;
