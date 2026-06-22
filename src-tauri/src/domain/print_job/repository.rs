use super::aggregate::PrintJob;
use super::errors::PrintJobError;
use super::value_objects::{PrintJobId, PrintStatus};

pub trait PrintJobRepository: Send + Sync {
    fn save(&self, job: &PrintJob) -> Result<(), PrintJobError>;

    fn update(&self, job: &PrintJob) -> Result<(), PrintJobError>;

    fn find_by_id(&self, id: &PrintJobId) -> Result<Option<PrintJob>, PrintJobError>;

    fn find_by_status(&self, status: &PrintStatus) -> Result<Vec<PrintJob>, PrintJobError>;

    fn find_by_slip_id(&self, slip_id: &str) -> Result<Vec<PrintJob>, PrintJobError>;

    fn find_all(&self) -> Result<Vec<PrintJob>, PrintJobError>;

    fn clear_terminal(&self) -> Result<u64, PrintJobError>;
}
