use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterResponse {
    pub id: String,
    pub name: String,
    pub status: String,       // "Online" | "Offline" | "Error"
    pub printer_type: String, // "Local" | "Network"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_default: Option<bool>,
}
