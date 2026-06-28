#[derive(Debug, Clone)]
pub struct Printer {
    pub name: String,
    pub device_id: String,
    pub status: String,
    pub printer_type: String,
    pub is_default: Option<bool>,
}
