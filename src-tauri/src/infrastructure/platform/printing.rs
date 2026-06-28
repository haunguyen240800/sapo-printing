//! `PrintService` adapter — wires the platform graphics backend to a
//! `RenderStrategy`.
//!
//! Lives under `platform::` because the underlying graphics backend is
//! platform-specific (Win32 GDI / CoreGraphics / Cairo). The rendering
//! strategy itself is platform-agnostic (PDFium), but the act of "open a
//! device context, render, close" is the platform abstraction.

use std::path::Path;
use std::sync::Arc;

use crate::application::ports::PrintService;
use crate::domain::print_job::PrintJobSettings;
use crate::infrastructure::integrations::pdf_engine::renderer::RenderStrategy;
use crate::infrastructure::platform::printer_api::backend::GraphicsBackendFactory;
use crate::shared::errors::InfrastructureError;

/// Default `PrintService` — creates a fresh native graphics backend per job
/// and delegates rendering to the injected strategy.
pub struct DefaultPrintService {
    render_strategy: Arc<dyn RenderStrategy>,
}

impl DefaultPrintService {
    pub fn new(render_strategy: Arc<dyn RenderStrategy>) -> Self {
        Self { render_strategy }
    }
}

impl PrintService for DefaultPrintService {
    fn print(
        &self,
        pdf_path: &str,
        printer_name: &str,
        settings: &PrintJobSettings,
    ) -> Result<(), InfrastructureError> {
        let mut backend = GraphicsBackendFactory::create();
        backend
            .begin_document(printer_name, "Sapo Print Job", None)
            .map_err(InfrastructureError::RenderError)?;

        let render_result = self.render_strategy.render(pdf_path, settings, &mut *backend);

        backend.end_document();

        render_result
    }

    fn save_to_path(
        &self,
        pdf_path: &Path,
        output_path: &str,
    ) -> Result<(), InfrastructureError> {
        std::fs::copy(pdf_path, output_path).map_err(|e| InfrastructureError::PrinterError {
            reason: format!("Failed to save PDF to {}: {}", output_path, e),
        })?;
        Ok(())
    }
}
