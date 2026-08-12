pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub trait DomainEvent: Send + Sync + std::fmt::Debug {
    fn event_name(&self) -> &'static str;
    fn serialize_payload(&self) -> String {
        "{}".to_string()
    }
}
