use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintJobFilterRequest {
    pub status: Option<String>,
    pub printer_name: Option<String>,
    pub from_date: Option<i64>, // Unix timestamp
    pub to_date: Option<i64>,
}
