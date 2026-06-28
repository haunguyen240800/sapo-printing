//! `PrintTask` — entity representing one unit of printable work inside a `PrintJob`.
//!
//! In the current model a `PrintJob` owns a single `PrintTask` (1-1). The 1-N
//! relationship (one job → many documents) is prepared but not yet wired up.
//! `PrintTask` carries the document reference and the rendered-file location;
//! the parent `PrintJob` owns the overall lifecycle state (PrintStatus).
//!
//! Mutation rules:
//! - Created via the parent aggregate (`PrintJob::new*`)
//! - `set_output_path` invoked by the aggregate during the render step

use serde::{Deserialize, Serialize};

use super::super::value_objects::PrintTaskId;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrintTask {
    id: PrintTaskId,
    // TODO(1-N PrintJob): replace `pdf_url` with reference to a `Document` aggregate
    pdf_url: String,
    output_path: Option<String>,
}

impl PrintTask {
    pub fn new(pdf_url: String, output_path: Option<String>) -> Self {
        Self {
            id: PrintTaskId::new(),
            pdf_url,
            output_path,
        }
    }

    /// Re-hydrate from persistence. Used by the aggregate's `reconstruct` path.
    pub fn reconstruct(id: PrintTaskId, pdf_url: String, output_path: Option<String>) -> Self {
        Self {
            id,
            pdf_url,
            output_path,
        }
    }

    pub fn id(&self) -> &PrintTaskId {
        &self.id
    }

    pub fn pdf_url(&self) -> &str {
        &self.pdf_url
    }

    pub fn output_path(&self) -> Option<&str> {
        self.output_path.as_deref()
    }
}
