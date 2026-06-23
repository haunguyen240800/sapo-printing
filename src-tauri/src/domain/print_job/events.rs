use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use super::value_objects::JobId;

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Trait for all domain events emitted by the PrintJob aggregate.
pub trait DomainEvent: Send + std::fmt::Debug {
    fn event_type(&self) -> &str;
    fn aggregate_id(&self) -> &JobId;
    /// Serialize this event to a JSON string.
    fn serialize_payload(&self) -> String;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrintJobCreated {
    pub job_id: JobId,
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
    fn event_type(&self) -> &str {
        "PrintJobCreated"
    }
    fn aggregate_id(&self) -> &JobId {
        &self.job_id
    }
    fn serialize_payload(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrintJobQueued {
    pub job_id: JobId,
    pub timestamp: u64,
}

impl PrintJobQueued {
    pub fn new(job_id: JobId) -> Self {
        Self {
            job_id,
            timestamp: now_unix(),
        }
    }
}

impl DomainEvent for PrintJobQueued {
    fn event_type(&self) -> &str {
        "PrintJobQueued"
    }
    fn aggregate_id(&self) -> &JobId {
        &self.job_id
    }
    fn serialize_payload(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrintJobDownloaded {
    pub job_id: JobId,
    pub timestamp: u64,
}

impl PrintJobDownloaded {
    pub fn new(job_id: JobId) -> Self {
        Self {
            job_id,
            timestamp: now_unix(),
        }
    }
}

impl DomainEvent for PrintJobDownloaded {
    fn event_type(&self) -> &str {
        "PrintJobDownloaded"
    }
    fn aggregate_id(&self) -> &JobId {
        &self.job_id
    }
    fn serialize_payload(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrintJobSubmitted {
    pub job_id: JobId,
    pub timestamp: u64,
}

impl PrintJobSubmitted {
    pub fn new(job_id: JobId) -> Self {
        Self {
            job_id,
            timestamp: now_unix(),
        }
    }
}

impl DomainEvent for PrintJobSubmitted {
    fn event_type(&self) -> &str {
        "PrintJobSubmitted"
    }
    fn aggregate_id(&self) -> &JobId {
        &self.job_id
    }
    fn serialize_payload(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrintJobPrinting {
    pub job_id: JobId,
    pub timestamp: u64,
}

impl PrintJobPrinting {
    pub fn new(job_id: JobId) -> Self {
        Self {
            job_id,
            timestamp: now_unix(),
        }
    }
}

impl DomainEvent for PrintJobPrinting {
    fn event_type(&self) -> &str {
        "PrintJobPrinting"
    }
    fn aggregate_id(&self) -> &JobId {
        &self.job_id
    }
    fn serialize_payload(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrintJobCompleted {
    pub job_id: JobId,
    pub timestamp: u64,
}

impl PrintJobCompleted {
    pub fn new(job_id: JobId) -> Self {
        Self {
            job_id,
            timestamp: now_unix(),
        }
    }
}

impl DomainEvent for PrintJobCompleted {
    fn event_type(&self) -> &str {
        "PrintJobCompleted"
    }
    fn aggregate_id(&self) -> &JobId {
        &self.job_id
    }
    fn serialize_payload(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

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
    fn event_type(&self) -> &str {
        "PrintJobFailed"
    }
    fn aggregate_id(&self) -> &JobId {
        &self.job_id
    }
    fn serialize_payload(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrintJobCancelled {
    pub job_id: JobId,
    pub timestamp: u64,
}

impl PrintJobCancelled {
    pub fn new(job_id: JobId) -> Self {
        Self {
            job_id,
            timestamp: now_unix(),
        }
    }
}

impl DomainEvent for PrintJobCancelled {
    fn event_type(&self) -> &str {
        "PrintJobCancelled"
    }
    fn aggregate_id(&self) -> &JobId {
        &self.job_id
    }
    fn serialize_payload(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_job_created_event() {
        let job_id = JobId::new();
        let event = PrintJobCreated::new(
            job_id.clone(),
            "https://s3.example.com/doc.pdf".to_string(),
            "HP_LaserJet".to_string(),
        );
        assert_eq!(event.event_type(), "PrintJobCreated");
        assert_eq!(event.aggregate_id(), &job_id);
        assert!(event.timestamp > 0);
        assert_eq!(event.pdf_url, "https://s3.example.com/doc.pdf");
        assert_eq!(event.printer_name, "HP_LaserJet");
    }

    #[test]
    fn test_print_job_queued_event() {
        let job_id = JobId::new();
        let event = PrintJobQueued::new(job_id.clone());
        assert_eq!(event.event_type(), "PrintJobQueued");
        assert_eq!(event.aggregate_id(), &job_id);
    }

    #[test]
    fn test_print_job_downloaded_event() {
        let job_id = JobId::new();
        let event = PrintJobDownloaded::new(job_id.clone());
        assert_eq!(event.event_type(), "PrintJobDownloaded");
        assert_eq!(event.aggregate_id(), &job_id);
    }

    #[test]
    fn test_print_job_submitted_event() {
        let job_id = JobId::new();
        let event = PrintJobSubmitted::new(job_id.clone());
        assert_eq!(event.event_type(), "PrintJobSubmitted");
        assert_eq!(event.aggregate_id(), &job_id);
    }

    #[test]
    fn test_print_job_completed_event() {
        let job_id = JobId::new();
        let event = PrintJobCompleted::new(job_id.clone());
        assert_eq!(event.event_type(), "PrintJobCompleted");
        assert_eq!(event.aggregate_id(), &job_id);
    }

    #[test]
    fn test_print_job_failed_event() {
        let job_id = JobId::new();
        let event = PrintJobFailed::new(job_id.clone(), "Network timeout".to_string(), 1);
        assert_eq!(event.event_type(), "PrintJobFailed");
        assert_eq!(event.aggregate_id(), &job_id);
        assert_eq!(event.reason, "Network timeout");
        assert_eq!(event.retry_count, 1);
    }

    #[test]
    fn test_print_job_printing_event() {
        let job_id = JobId::new();
        let event = PrintJobPrinting::new(job_id.clone());
        assert_eq!(event.event_type(), "PrintJobPrinting");
        assert_eq!(event.aggregate_id(), &job_id);
        assert!(event.timestamp > 0);
    }

    #[test]
    fn test_print_job_cancelled_event() {
        let job_id = JobId::new();
        let event = PrintJobCancelled::new(job_id.clone());
        assert_eq!(event.event_type(), "PrintJobCancelled");
        assert_eq!(event.aggregate_id(), &job_id);
        assert!(event.timestamp > 0);
    }

    #[test]
    fn test_events_are_debug() {
        let job_id = JobId::new();
        let event = PrintJobCreated::new(job_id, "url".to_string(), "printer".to_string());
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("PrintJobCreated"));
    }

    #[test]
    fn test_events_are_clone() {
        let job_id = JobId::new();
        let event = PrintJobFailed::new(job_id, "error".to_string(), 2);
        let cloned = event.clone();
        assert_eq!(cloned.reason, event.reason);
        assert_eq!(cloned.retry_count, event.retry_count);
    }
}
