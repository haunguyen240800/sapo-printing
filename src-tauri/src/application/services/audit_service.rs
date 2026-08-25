use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::application::ports::{EventStore, StoredEventData};
use crate::domain::print_job::PrintJobError;

pub fn get_audit_trail(
    store: &Arc<dyn EventStore>,
    aggregate_id: &str,
) -> Result<Vec<StoredEventData>, PrintJobError> {
    store.find_by_aggregate(aggregate_id)
}

pub fn cleanup_old_events(
    store: &Arc<dyn EventStore>,
    retention_days: u32,
) -> Result<u64, PrintJobError> {
    if retention_days == 0 {
        return Err(PrintJobError::RepositoryError {
            reason: "retention_days must be greater than 0".to_string(),
        });
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let cutoff = now - (retention_days as i64 * 86400);
    store.delete_events_before(cutoff)
}
