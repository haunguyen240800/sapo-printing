use super::aggregate::Printer;
use super::errors::PrinterDomainError;
use super::value_objects::PrinterName;

/// Repository trait for `Printer` aggregate persistence.
///
/// Implementations live in the Infrastructure layer.
/// Domain layer only defines this contract.
pub trait PrinterRepository {
    /// Persist a new printer record.
    fn save(&self, printer: &Printer) -> Result<(), PrinterDomainError>;

    /// Retrieve all known printers.
    fn find_all(&self) -> Result<Vec<Printer>, PrinterDomainError>;

    /// Find a printer by its OS-visible name. Returns `None` if not found.
    fn find_by_name(&self, name: &PrinterName) -> Result<Option<Printer>, PrinterDomainError>;
}
