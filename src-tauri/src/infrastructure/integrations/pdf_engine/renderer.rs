use crate::domain::models::PrintJobSettings;
use crate::infrastructure::platform::printer_api::backend::GraphicsBackend;

// Strategy trait to allow different rendering methods (e.g. Bitmap vs Native)
pub trait RenderStrategy {
    fn render(
        &self,
        pdf_path: &str,
        settings: &PrintJobSettings,
        backend: &mut dyn GraphicsBackend,
    );
}
