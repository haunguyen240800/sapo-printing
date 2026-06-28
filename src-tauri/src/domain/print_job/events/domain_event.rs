use crate::domain::print_job::value_objects::PrintJobId;

pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub trait DomainEvent: Send + std::fmt::Debug {
    fn event_type(&self) -> &str;
    fn aggregate_id(&self) -> &PrintJobId;
    fn serialize_payload(&self) -> String;
}
