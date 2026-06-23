// Entry point for Tauri application
// This file initializes the Tauri runtime and registers commands

// Prevents additional console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use sapo_printer::infrastructure::database::{DbPool, run_migrations, SqlitePrinterRepository};
use sapo_printer::infrastructure::printer::PrinterManager;
use sapo_printer::interface::tauri::dtos::printer_dto::{PrinterConfigDto, PrinterDto, PrinterStatusDto};

#[cfg(target_os = "windows")]
use sapo_printer::infrastructure::printer::windows::Win32PrinterManager;

#[cfg(not(target_os = "windows"))]
use sapo_printer::infrastructure::printer::cups::CupsPrinterManager;

/// List all available printers (discovered + saved configs)
#[tauri::command]
fn list_printers(app_ctx: tauri::State<AppContextState>) -> Result<Vec<PrinterDto>, String> {
    // 1. Discover printers from OS
    let discovered = app_ctx.printer_manager.discover_printers();

    // 2. Load saved configs from repository
    let _saved_printers = app_ctx.printer_repo.find_all()
        .map_err(|e| format!("Lỗi truy vấn cơ sở dữ liệu: {}", e))?;

    // 3. Map to DTOs - merge discovered with saved configs
    let dtos: Vec<PrinterDto> = discovered.iter().map(|printer| {
        // Check if this printer has saved config with is_default flag
        // For now, we'll just map the discovered printer
        PrinterDto {
            name: printer.name().as_str().to_string(),
            device_id: printer.name().as_str().to_string(), // device_id = printer_name
            status: match printer.status() {
                sapo_printer::domain::printer::PrinterStatus::Online => "Online".to_string(),
                sapo_printer::domain::printer::PrinterStatus::Offline => "Offline".to_string(),
                sapo_printer::domain::printer::PrinterStatus::Error => "Error".to_string(),
            },
            printer_type: match printer.printer_type() {
                sapo_printer::domain::printer::PrinterType::Local => "Local".to_string(),
                sapo_printer::domain::printer::PrinterType::Network => "Network".to_string(),
            },
            is_default: None, // TODO: merge with saved configs to get is_default
        }
    }).collect();

    // TODO: Merge with saved_printers to set is_default flag
    // For now, just return discovered printers

    Ok(dtos)
}

/// Save printer configuration
#[tauri::command]
fn save_printer_config(config: PrinterConfigDto, app_ctx: tauri::State<AppContextState>) -> Result<(), String> {
    // 1. Validate config fields
    // Paper size validation
    if config.paper_size.is_empty() {
        return Err("Khổ giấy không được để trống".to_string());
    }

    // Custom dimensions validation (50-500mm range)
    if config.paper_size == "Custom" {
        if let Some(width) = config.paper_width {
            if !(50..=500).contains(&width) {
                return Err("Chiều rộng giấy phải trong khoảng 50-500mm".to_string());
            }
        } else {
            return Err("Chiều rộng giấy bắt buộc khi chọn khổ Custom".to_string());
        }

        if let Some(height) = config.paper_height {
            if !(50..=500).contains(&height) {
                return Err("Chiều cao giấy phải trong khoảng 50-500mm".to_string());
            }
        } else {
            return Err("Chiều cao giấy bắt buộc khi chọn khổ Custom".to_string());
        }
    }

    // Margins validation (0-100mm range)
    if config.margin_left > 100 {
        return Err("Lề trái phải trong khoảng 0-100mm".to_string());
    }
    if config.margin_right > 100 {
        return Err("Lề phải phải trong khoảng 0-100mm".to_string());
    }
    if config.margin_top > 100 {
        return Err("Lề trên phải trong khoảng 0-100mm".to_string());
    }
    if config.margin_bottom > 100 {
        return Err("Lề dưới phải trong khoảng 0-100mm".to_string());
    }

    // Printer name validation
    if config.printer_name.is_empty() {
        return Err("Tên máy in không được để trống".to_string());
    }

    // 2. Load or create Printer from repository
    use sapo_printer::domain::printer::{PrinterName, PrinterType};

    let printer_name = PrinterName::new(config.printer_name.clone());
    let printer = match app_ctx.printer_repo.find_by_name(&printer_name)
        .map_err(|e| format!("Lỗi truy vấn cơ sở dữ liệu: {}", e))? {
        Some(p) => p,
        None => {
            // Create new printer if not found
            sapo_printer::domain::printer::Printer::new(printer_name, PrinterType::Local)
        }
    };

    // 3. Note: Config fields (paper size, margins, etc.) are stored in printer_configs table
    //    but are NOT part of the Printer aggregate domain model. The repository handles
    //    these separately. We just save the Printer aggregate.

    // 4. Save printer (repository will handle config fields via UPSERT)
    app_ctx.printer_repo.save(&printer)
        .map_err(|e| format!("Lỗi lưu cấu hình: {}", e))?;

    // TODO: In future, extend repository to accept config parameters
    // For now, the basic save() works because repository has default values

    Ok(())
}

/// Get current printer status
#[tauri::command]
fn get_printer_status(name: String, app_ctx: tauri::State<AppContextState>) -> Result<PrinterStatusDto, String> {
    // 1. Access PrinterManager from AppContext
    // 2. Call printer_manager.get_status(name)
    let status = app_ctx.printer_manager.get_status(&name);

    // 3. Map PrinterStatus enum to String
    let status_str = match status {
        sapo_printer::domain::printer::PrinterStatus::Online => "Online".to_string(),
        sapo_printer::domain::printer::PrinterStatus::Offline => "Offline".to_string(),
        sapo_printer::domain::printer::PrinterStatus::Error => "Error".to_string(),
    };

    // 4. Return PrinterStatusDto
    Ok(PrinterStatusDto {
        status: status_str,
    })
}

fn main() {
    // 1. Ensure ~/.sapo-printer/ data directory exists
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let data_dir = std::path::PathBuf::from(&home).join(".sapo-printer");
    std::fs::create_dir_all(&data_dir).unwrap_or_else(|e| {
        eprintln!("Cannot create data directory: {e}");
        std::process::exit(1);
    });
    let db_path = data_dir.join("config.db");
    let db_path_str = db_path.to_str().unwrap_or_else(|| {
        eprintln!("Database path contains non-UTF-8 characters");
        std::process::exit(1);
    });

    // 2. Initialize database connection
    let pool = DbPool::new(db_path_str).unwrap_or_else(|e| {
        eprintln!("Database init failed: {e}");
        std::process::exit(1);
    });

    // 3. Run migrations
    {
        let mut conn = pool.get();
        run_migrations(&mut conn).unwrap_or_else(|e| {
            eprintln!("Migration failed: {e}");
            std::process::exit(1);
        });
    }

    // 4. Initialize AppContext dependencies
    let printer_repo = Arc::new(SqlitePrinterRepository::new(pool.get_arc()));

    #[cfg(target_os = "windows")]
    let printer_manager: Arc<dyn PrinterManager> = Arc::new(Win32PrinterManager::new());

    #[cfg(not(target_os = "windows"))]
    let printer_manager: Arc<dyn PrinterManager> = Arc::new(CupsPrinterManager::new());

    // TODO: Wire job_repo and event_bus when those are implemented
    // For now, we'll create a minimal AppContext structure inline

    // 5. Start Tauri — AppContext registered as managed state
    tauri::Builder::default()
        .manage(AppContextState {
            printer_repo,
            printer_manager,
        })
        .invoke_handler(tauri::generate_handler![
            list_printers,
            save_printer_config,
            get_printer_status
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("Failed to start Tauri application:");
            eprintln!("  {e}");
            std::process::exit(1);
        });
}

// Temporary state structure until AppContext is fully wired
struct AppContextState {
    printer_repo: Arc<dyn sapo_printer::domain::printer::PrinterRepository>,
    printer_manager: Arc<dyn PrinterManager>,
}
