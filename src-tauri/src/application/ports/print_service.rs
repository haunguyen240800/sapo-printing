//! PrintService port — application abstraction over the end-to-end print pipeline.
//!
//! Use cases (specifically `ProcessPrintJobUseCase`) depend on this trait to
//! either render-and-spool a document to a printer or save it directly to a
//! caller-chosen path (for "Print to PDF" virtual printers). The concrete
//! implementation (in `infrastructure::printing`) composes the graphics backend
//! factory and the PDF render strategy.

use std::path::Path;

use crate::domain::print_job::PrintJobSettings;
use crate::shared::errors::InfrastructureError;

/// Contract for sending a downloaded PDF to a printer or saving it to disk.
pub trait PrintService: Send + Sync {
    /// Render `pdf_path` and submit it to `printer_name` using `settings`.
    ///
    /// Returns `Ok(())` once the document has been fully spooled. Errors must
    /// be surfaced through `InfrastructureError` so the worker can decide
    /// whether to retry or fail the job permanently.
    fn print(
        &self,
        pdf_path: &str,
        printer_name: &str,
        settings: &PrintJobSettings,
    ) -> Result<(), InfrastructureError>;

    /// Save the prepared PDF to `output_path` instead of spooling it to a
    /// printer driver. Used when the target is a "Print to PDF" virtual
    /// printer and the caller chose a destination file.
    fn save_to_path(
        &self,
        pdf_path: &Path,
        output_path: &str,
    ) -> Result<(), InfrastructureError>;
}
