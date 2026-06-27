use serde::{Deserialize, Serialize};

use crate::domain::models::job_id::JobId;

pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub trait DomainEvent: Send + std::fmt::Debug {
    fn event_type(&self) -> &str;
    fn aggregate_id(&self) -> &JobId;
    fn serialize_payload(&self) -> String;
}
