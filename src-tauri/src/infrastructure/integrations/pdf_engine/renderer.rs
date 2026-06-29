use crate::domain::print_job::PrintJobSettings;
use crate::infrastructure::platform::printer_api::backend::GraphicsBackend;
use crate::shared::errors::InfrastructureError;

/// Strategy trait to allow different rendering methods (e.g. Bitmap vs Native).
///
/// Must be `Send + Sync` so it can be shared across threads via `Arc<dyn RenderStrategy>`.
///
/// Returns `Err(InfrastructureError::RenderError)` if PDFium fails to load or the
/// document is unrenderable. Callers MUST propagate the error so failed jobs are
/// not silently marked as completed.
pub trait RenderStrategy: Send + Sync {
    /// Render all pages of the PDF to the printer.
    ///
    /// The implementation is responsible for the full document lifecycle: it must
    /// call `backend.begin_document()`/`end_document()` for each page (or batch)
    /// so that each label is submitted as a separate spooler document. This is
    /// required for thermal label printers whose drivers reset DC state between
    /// pages and do not reliably handle multi-page GDI documents.
    fn render(
        &self,
        pdf_path: &str,
        printer_name: &str,
        settings: &PrintJobSettings,
        backend: &mut dyn GraphicsBackend,
    ) -> Result<(), InfrastructureError>;
}
