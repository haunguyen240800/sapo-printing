//! `PrintTaskId` — identity of a `PrintTask` entity inside a `PrintJob` aggregate.
//!
//! UUID-based local identity. Not persisted separately yet (the 1-1 PrintJob–PrintTask
//! relationship means task identity is currently regenerated on aggregate reconstruction);
//! the explicit type prepares the codebase for the future 1-N PrintTask model.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrintTaskId(Uuid);

impl PrintTaskId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for PrintTaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PrintTaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
