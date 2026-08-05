use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterDto {
    pub id: String,
    pub name: String,
    pub status: String,
    pub printer_type: String,
    pub is_default: bool,
}
