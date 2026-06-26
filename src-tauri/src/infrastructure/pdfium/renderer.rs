use crate::domain::settings::PrintSettings;
use crate::infrastructure::graphics::backend::GraphicsBackend;

// Strategy trait to allow different rendering methods (e.g. Bitmap vs Native)
pub trait RenderStrategy {
    fn render(
        &self,
        pdf_path: &str,
        settings: &PrintSettings,
        backend: &mut dyn GraphicsBackend,
    );
}
