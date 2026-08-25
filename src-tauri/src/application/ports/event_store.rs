use crate::domain::print_job::PrintJobError;
use crate::domain::print_job::events::DomainEvent;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StoredEventData {
    pub id: i64,
    pub aggregate_id: String,
    pub sequence_number: i64,
    pub event_type: String,
    pub payload: String,
    pub timestamp: i64,
}

pub trait EventStore: Send + Sync {
    fn save_all(
        &self,
        aggregate_id: &str,
        events: &[Box<dyn DomainEvent>],
    ) -> Result<(), PrintJobError>;

    fn find_by_aggregate(&self, aggregate_id: &str) -> Result<Vec<StoredEventData>, PrintJobError>;

    fn delete_events_before(&self, cutoff_timestamp: i64) -> Result<u64, PrintJobError>;
}
