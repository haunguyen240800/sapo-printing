use super::domain_event::{DomainEvent, now_unix};
use crate::domain::print_job::value_objects::PrintJobId;
use serde::{Deserialize, Serialize};

macro_rules! define_status_event {
    ($name:ident, $event_type:expr_2021) => {
        #[derive(Clone, Debug, Serialize, Deserialize)]
        pub struct $name {
            pub job_id: PrintJobId,
            pub timestamp: u64,
        }

        impl $name {
            pub fn new(job_id: PrintJobId) -> Self {
                Self {
                    job_id,
                    timestamp: now_unix(),
                }
            }
        }

        impl DomainEvent for $name {
            fn event_name(&self) -> &'static str {
                $event_type
            }
            fn serialize_payload(&self) -> String {
                serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
            }
        }
    };
}

define_status_event!(PrintJobQueued, "PrintJobQueued");
define_status_event!(PrintJobDownloaded, "PrintJobDownloaded");
define_status_event!(PrintJobSubmitted, "PrintJobSubmitted");
define_status_event!(PrintJobPrinting, "PrintJobPrinting");
define_status_event!(PrintJobCompleted, "PrintJobCompleted");
define_status_event!(PrintJobCancelled, "PrintJobCancelled");
