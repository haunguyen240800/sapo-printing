use serde::{Deserialize, Serialize};
use super::domain_event::{now_unix, DomainEvent};
use crate::domain::models::job_id::JobId;
use crate::domain::common::aggregate::DomainEvent as CommonDomainEvent;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrintJobCreated {
    pub job_id: JobId,
    // TODO(1-N PrintJob): Change `pdf_url` to `documents: Vec<crate::domain::models::document::Document>`
    pub pdf_url: String,
    pub printer_name: String,
    pub timestamp: u64,
}

impl PrintJobCreated {
    pub fn new(job_id: JobId, pdf_url: String, printer_name: String) -> Self {
        Self {
            job_id,
            pdf_url,
            printer_name,
            timestamp: now_unix(),
        }
    }
}

impl DomainEvent for PrintJobCreated {
    fn event_type(&self) -> &str { "PrintJobCreated" }
    fn aggregate_id(&self) -> &JobId { &self.job_id }
    fn serialize_payload(&self) -> String { serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string()) }
}
impl CommonDomainEvent for PrintJobCreated { fn event_name(&self) -> &'static str { "PrintJobCreated" } }
