use crate::domain::models::PrintJob;
use crate::domain::models::DomainError;
use crate::domain::models::JobId;
use crate::domain::models::PrintStatus;

/// Repository trait for PrintJob aggregate persistence.
///
/// Implementations (e.g., SQLite) live in the infrastructure layer.
/// The domain layer defines only the contract — no storage details.
pub trait PrintJobRepository: Send + Sync {
    /// Persists a new PrintJob.
    fn save(&self, job: &PrintJob) -> Result<(), DomainError>;

    /// Updates an existing PrintJob.
    fn update(&self, job: &PrintJob) -> Result<(), DomainError>;

    /// Finds a PrintJob by its unique ID.
    /// Returns None if not found.
    fn find_by_id(&self, id: &JobId) -> Result<Option<PrintJob>, DomainError>;

    /// Finds all PrintJobs with the given status.
    fn find_by_status(&self, status: &PrintStatus) -> Result<Vec<PrintJob>, DomainError>;

    /// Finds all PrintJobs in the database.
    fn find_all(&self) -> Result<Vec<PrintJob>, DomainError>;
}
