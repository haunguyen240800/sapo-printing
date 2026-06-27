use serde::{Deserialize, Serialize};
use super::domain_event::{now_unix, DomainEvent};
use crate::domain::models::job_id::JobId;
use crate::domain::common::aggregate::DomainEvent as CommonDomainEvent;

macro_rules! define_status_event {
    ($name:ident, $event_type:expr) => {
        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct $name {
            pub job_id: JobId,
            pub timestamp: u64,
        }

        impl $name {
            pub fn new(job_id: JobId) -> Self {
                Self {
                    job_id,
                    timestamp: now_unix(),
                }
            }
        }

        impl DomainEvent for $name {
            fn event_type(&self) -> &str { $event_type }
            fn aggregate_id(&self) -> &JobId { &self.job_id }
            fn serialize_payload(&self) -> String { serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string()) }
        }
        impl CommonDomainEvent for $name {
            fn event_name(&self) -> &'static str { $event_type }
            fn serialize_payload(&self) -> String { serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string()) }
        }
    };
}

define_status_event!(PrintJobQueued, "PrintJobQueued");
define_status_event!(PrintJobDownloaded, "PrintJobDownloaded");
define_status_event!(PrintJobSubmitted, "PrintJobSubmitted");
define_status_event!(PrintJobPrinting, "PrintJobPrinting");
define_status_event!(PrintJobCompleted, "PrintJobCompleted");
define_status_event!(PrintJobCancelled, "PrintJobCancelled");
