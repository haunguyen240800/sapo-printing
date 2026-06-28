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
    fn render(
        &self,
        pdf_path: &str,
        settings: &PrintJobSettings,
        backend: &mut dyn GraphicsBackend,
    ) -> Result<(), InfrastructureError>;
}
