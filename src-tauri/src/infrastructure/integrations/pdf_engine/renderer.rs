use crate::domain::print_job::PrintJobSettings;
use crate::infrastructure::platform::printer_api::backend::GraphicsBackend;
use crate::shared::errors::InfrastructureError;

pub trait RenderStrategy: Send + Sync {
    fn render(
        &self,
        pdf_path: &str,
        printer_name: &str,
        settings: &PrintJobSettings,
        backend: &mut dyn GraphicsBackend,
    ) -> Result<(), InfrastructureError>;
}
