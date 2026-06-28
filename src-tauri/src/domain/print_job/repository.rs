use super::aggregate::PrintJob;
use super::errors::PrintJobError;
use super::value_objects::{PrintJobId, PrintStatus};

/// Repository trait for PrintJob aggregate persistence.
///
/// Implementations (e.g., SQLite) live in the infrastructure layer.
/// The domain layer defines only the contract — no storage details.
pub trait PrintJobRepository: Send + Sync {
    /// Persists a new PrintJob.
    fn save(&self, job: &PrintJob) -> Result<(), PrintJobError>;

    /// Updates an existing PrintJob.
    fn update(&self, job: &PrintJob) -> Result<(), PrintJobError>;

    /// Finds a PrintJob by its unique ID.
    /// Returns None if not found.
    fn find_by_id(&self, id: &PrintJobId) -> Result<Option<PrintJob>, PrintJobError>;

    /// Finds all PrintJobs with the given status.
    fn find_by_status(&self, status: &PrintStatus) -> Result<Vec<PrintJob>, PrintJobError>;

    /// Finds all PrintJobs in the database.
    fn find_all(&self) -> Result<Vec<PrintJob>, PrintJobError>;
}
