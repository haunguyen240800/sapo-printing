use tauri::State;

use crate::AppContextState;
use crate::application::models::{PrinterConfigRequest, PrinterConfigResponse, PrinterResponse, PrinterStatusResponse};

#[tauri::command]
pub fn list_printers(ctx: State<'_, AppContextState>) -> Result<Vec<PrinterResponse>, String> {
    ctx.list_printers_uc
        .clone()
        .execute()
        .map_err(|e| format!("{:?}", e))
}

#[tauri::command]
pub fn save_printer_config(
    config: PrinterConfigRequest,
    _app_ctx: State<'_, AppContextState>,
) -> Result<(), String> {
    use crate::infrastructure::configs::app::app_print_config;

    if config.paper_size.is_empty() {
        return Err("Khổ giấy không được để trống".to_string());
    }

    if config.paper_width > 0.0 && !(50.0..=500.0).contains(&config.paper_width) {
        return Err("Chiều rộng giấy phải trong khoảng 50-500mm".to_string());
    }

    if config.paper_height > 0.0 && !(50.0..=500.0).contains(&config.paper_height) {
        return Err("Chiều cao giấy phải trong khoảng 50-500mm".to_string());
    }

    if config.paper_size == "Custom" {
        if config.paper_width == 0.0 {
            return Err("Chiều rộng giấy bắt buộc khi chọn khổ Custom".to_string());
        }
        if config.paper_height == 0.0 {
            return Err("Chiều cao giấy bắt buộc khi chọn khổ Custom".to_string());
        }
    }

    if config.printer_name.is_empty() {
        return Err("Tên máy in không được để trống".to_string());
    }

    let valid_color_modes = ["RGB", "ARGB", "BGR", "GRAY", "BINARY"];
    if !valid_color_modes.contains(&config.color_mode.as_str()) {
        return Err(format!(
            "Loại ảnh in không hợp lệ: '{}'. Chỉ chấp nhận: RGB, ARGB, BGR, GRAY, BINARY",
            config.color_mode
        ));
    }

    let print_config = app_print_config::AppPrintConfig {
        printer_name: config.printer_name,
        paper_size: config.paper_size,
        paper_width: Some(config.paper_width),
        paper_height: Some(config.paper_height),
        orientation: config.orientation,
        margin_left: config.margin_left,
        margin_right: config.margin_right,
        margin_top: config.margin_top,
        margin_bottom: config.margin_bottom,
        color_mode: config.color_mode,
        print_as_image: config.print_as_image,
        enable_buffer: config.enable_buffer,
        buffer_size_kb: config.buffer_size_kb,
    };

    app_print_config::save_config(&print_config)?;

    Ok(())
}

#[tauri::command]
pub fn get_printer_config(
    _app_ctx: State<'_, AppContextState>,
) -> Result<PrinterConfigResponse, String> {
    use crate::infrastructure::configs::app::app_print_config;

    let config = app_print_config::load_config()?;

    match config {
        Some(cfg) => Ok(PrinterConfigResponse {
            printer_name: cfg.printer_name,
            paper_size: cfg.paper_size,
            paper_width: cfg.paper_width,
            paper_height: cfg.paper_height,
            orientation: cfg.orientation,
            margin_left: cfg.margin_left,
            margin_right: cfg.margin_right,
            margin_top: cfg.margin_top,
            margin_bottom: cfg.margin_bottom,
            color_mode: cfg.color_mode,
            print_as_image: cfg.print_as_image,
            enable_buffer: cfg.enable_buffer,
            buffer_size_kb: cfg.buffer_size_kb,
        }),
        None => Ok(PrinterConfigResponse {
            printer_name: String::new(),
            paper_size: String::new(),
            paper_width: None,
            paper_height: None,
            orientation: String::new(),
            margin_left: 0.0,
            margin_right: 0.0,
            margin_top: 0.0,
            margin_bottom: 0.0,
            color_mode: String::new(),
            print_as_image: false,
            enable_buffer: false,
            buffer_size_kb: None,
        }),
    }
}

#[tauri::command]
pub fn get_printer_status(_name: String) -> Result<PrinterStatusResponse, String> {
    Ok(PrinterStatusResponse {
        status: "Online".to_string(),
    })
}

#[derive(serde::Serialize)]
pub struct PrinterCategoryResult {
    category: String,
    needs_rendering: bool,
    needs_save_dialog: bool,
    description: String,
}

#[tauri::command]
pub fn detect_printer_category(printer_name: String) -> Result<PrinterCategoryResult, String> {
    let printer_lower = printer_name.to_lowercase();

    let pdf_patterns = [
        "microsoft print to pdf",
        "print to pdf",
        "save as pdf",
        "pdf printer",
        "adobe pdf",
        "foxit reader pdf printer",
        "nitro pdf creator",
        "cutepdf writer",
        "dopdf",
    ];

    let virtual_patterns = [
        "microsoft xps document writer",
        "microsoft print to image",
        "fax",
        "onenote",
        "send to onenote",
    ];

    let is_pdf = pdf_patterns.iter().any(|&p| printer_lower.contains(p));
    let is_virtual = virtual_patterns.iter().any(|&p| printer_lower.contains(p));

    if is_pdf {
        Ok(PrinterCategoryResult {
            category: "pdf".to_string(),
            needs_rendering: false,
            needs_save_dialog: true,
            description: "Print to PDF Virtual Printer".to_string(),
        })
    } else if is_virtual {
        Ok(PrinterCategoryResult {
            category: "virtual".to_string(),
            needs_rendering: false,
            needs_save_dialog: false,
            description: "Virtual/Image Printer".to_string(),
        })
    } else {
        Ok(PrinterCategoryResult {
            category: "physical".to_string(),
            needs_rendering: true,
            needs_save_dialog: false,
            description: "Physical Network/USB Printer".to_string(),
        })
    }
}
