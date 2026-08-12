use crate::application::errors::Error;
use crate::domain::print_job::{PaperSize, PrintJobSettings};

#[derive(Debug, Clone)]
pub struct PrintConfigSnapshot {
    pub printer_id: String,
    pub paper_size: String,
    pub paper_width: Option<u32>,
    pub paper_height: Option<u32>,
    pub orientation: String,
    pub margin_left: u32,
    pub margin_right: u32,
    pub margin_top: u32,
    pub margin_bottom: u32,
    pub color_mode: String,
    pub print_as_image: bool,
}

impl Default for PrintConfigSnapshot {
    fn default() -> Self {
        Self {
            printer_id: String::new(),
            paper_size: "A4".to_string(),
            paper_width: None,
            paper_height: None,
            orientation: "Portrait".to_string(),
            margin_left: 0,
            margin_right: 0,
            margin_top: 0,
            margin_bottom: 0,
            color_mode: "RGB".to_string(),
            print_as_image: false,
        }
    }
}

impl From<&PrintConfigSnapshot> for PrintJobSettings {
    fn from(config: &PrintConfigSnapshot) -> Self {
        PrintJobSettings {
            paper_size: PaperSize::from_parts(
                &config.paper_size,
                config.paper_width,
                config.paper_height,
            ),
            orientation: config.orientation.clone(),
            margin_left: config.margin_left,
            margin_right: config.margin_right,
            margin_top: config.margin_top,
            margin_bottom: config.margin_bottom,
            color_mode: config.color_mode.clone(),
            print_as_image: config.print_as_image,
            dpi: 300,
            copies: 1,
            rotate: 0.0,
        }
    }
}

/// Loads the persisted print configuration.
///
/// Returns `Ok(None)` when no configuration has been saved yet.
pub trait ConfigPort: Send + Sync {
    fn load_print_config(&self) -> Result<Option<PrintConfigSnapshot>, Error>;
}
