//! EventStore port — application-layer abstraction for event persistence.
//!
//! Defines the contract for persisting domain events. Infrastructure-layer
//! adapters (e.g. `SqliteEventStore`) implement this trait.
//!
//! This port lives in the `application` layer so use cases and the queue worker
//! can depend on the abstraction rather than the concrete SQLite implementation,
//! enabling straightforward mocking in unit tests.

use crate::domain::common::aggregate::DomainEvent;
use crate::domain::print_job::PrintJobError;

/// A persisted domain event as returned by the event store.
/// Kept here to avoid re-exporting from the infrastructure layer.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct StoredEventData {
    pub id: i64,
    pub aggregate_id: String,
    pub sequence_number: i64,
    pub event_type: String,
    pub payload: String,
    pub timestamp: i64,
    pub hmac: Option<String>,
}

/// Application-layer port for persisting and querying domain events.
///
/// Implementations are expected to be thread-safe (`Send + Sync`).
pub trait EventStore: Send + Sync {
    /// Persist a batch of domain events for the given aggregate in a single
    /// atomic transaction.
    fn save_all(
        &self,
        aggregate_id: &str,
        events: &[Box<dyn DomainEvent>],
    ) -> Result<(), PrintJobError>;

    /// Retrieve all events for an aggregate ordered by `sequence_number` ASC.
    fn find_by_aggregate(
        &self,
        aggregate_id: &str,
    ) -> Result<Vec<StoredEventData>, PrintJobError>;

    /// Delete events with a timestamp older than `cutoff_timestamp` (Unix epoch seconds).
    /// Returns the number of deleted rows.
    fn delete_events_before(&self, cutoff_timestamp: i64) -> Result<u64, PrintJobError>;
}
