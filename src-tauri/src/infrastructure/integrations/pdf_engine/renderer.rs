use crate::domain::models::PrintJobSettings;
use crate::infrastructure::platform::printer_api::backend::GraphicsBackend;

/// Strategy trait to allow different rendering methods (e.g. Bitmap vs Native).
///
/// Must be `Send + Sync` so it can be shared across threads via `Arc<dyn RenderStrategy>`.
pub trait RenderStrategy: Send + Sync {
    fn render(
        &self,
        pdf_path: &str,
        settings: &PrintJobSettings,
        backend: &mut dyn GraphicsBackend,
    );
}
