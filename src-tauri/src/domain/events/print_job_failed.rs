use serde::{Deserialize, Serialize};
use super::domain_event::{now_unix, DomainEvent};
use crate::domain::models::job_id::JobId;
use crate::domain::common::aggregate::DomainEvent as CommonDomainEvent;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrintJobFailed {
    pub job_id: JobId,
    pub reason: String,
    pub retry_count: u32,
    pub timestamp: u64,
}

impl PrintJobFailed {
    pub fn new(job_id: JobId, reason: String, retry_count: u32) -> Self {
        Self {
            job_id,
            reason,
            retry_count,
            timestamp: now_unix(),
        }
    }
}

impl DomainEvent for PrintJobFailed {
    fn event_type(&self) -> &str { "PrintJobFailed" }
    fn aggregate_id(&self) -> &JobId { &self.job_id }
    fn serialize_payload(&self) -> String { serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string()) }
}
impl CommonDomainEvent for PrintJobFailed { fn event_name(&self) -> &'static str { "PrintJobFailed" } }
