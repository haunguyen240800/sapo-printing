pub trait DomainEvent: Send + Sync + std::fmt::Debug {
    fn event_name(&self) -> &'static str;
    fn serialize_payload(&self) -> String {
        "{}".to_string()
    }
}

pub trait AggregateRoot {
    fn domain_events(&self) -> &[Box<dyn DomainEvent>];
    fn clear_domain_events(&mut self);
}
