use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterStatusResponse {
    pub status: String, // "Online" | "Offline" | "Error"
}
