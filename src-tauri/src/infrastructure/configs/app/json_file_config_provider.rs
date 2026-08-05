use crate::application::ports::{ConfigProvider, PrintConfigSnapshot};
use crate::infrastructure::configs::app::app_print_config;
use crate::shared::errors::InfrastructureError;

pub struct JsonFileConfigProvider;

impl JsonFileConfigProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsonFileConfigProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigProvider for JsonFileConfigProvider {
    fn load_print_config(&self) -> Result<Option<PrintConfigSnapshot>, InfrastructureError> {
        match app_print_config::load_config() {
            Ok(Some(cfg)) => Ok(Some(PrintConfigSnapshot {
                printer_id: cfg.printer_name,
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
            })),
            Ok(None) => Ok(None),
            Err(msg) => Err(InfrastructureError::ValidationError(msg)),
        }
    }
}
