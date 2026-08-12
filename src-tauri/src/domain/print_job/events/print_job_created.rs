use super::domain_event::{DomainEvent, now_unix};
use crate::domain::common::aggregate::DomainEvent as CommonDomainEvent;
use crate::domain::print_job::value_objects::{PrintJobId, PrinterId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrintJobCreated {
    pub job_id: PrintJobId,
    // TODO(1-N PrintJob): replace `pdf_url` with `documents: Vec<crate::domain::document::Document>`
    pub pdf_url: String,
    pub printer_id: PrinterId,
    pub timestamp: u64,
}

impl PrintJobCreated {
    pub fn new(job_id: PrintJobId, pdf_url: String, printer_id: PrinterId) -> Self {
        Self {
            job_id,
            pdf_url,
            printer_id,
            timestamp: now_unix(),
        }
    }
}

impl DomainEvent for PrintJobCreated {
    fn event_type(&self) -> &str {
        "PrintJobCreated"
    }
    fn aggregate_id(&self) -> &PrintJobId {
        &self.job_id
    }
    fn serialize_payload(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

impl CommonDomainEvent for PrintJobCreated {
    fn event_name(&self) -> &'static str {
        "PrintJobCreated"
    }
    fn serialize_payload(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}
