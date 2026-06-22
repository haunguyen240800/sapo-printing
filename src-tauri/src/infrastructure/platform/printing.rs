//! `PrintPort` adapter — wires the platform graphics backend to a
//! `RenderStrategy`.
//!
//! Lives under `platform::` because the underlying graphics backend is
//! platform-specific (Win32 GDI / CoreGraphics / Cairo). The rendering
//! strategy itself is platform-agnostic (PDFium), but the act of "open a
//! device context, render, close" is the platform abstraction.

use std::path::Path;
use std::sync::Arc;

use crate::application::errors::Error;
use crate::application::ports::PrintPort;
use crate::domain::print_job::PrintJobSettings;
use crate::infrastructure::errors::InfrastructureError;
use crate::infrastructure::integrations::pdf_engine::renderer::RenderStrategy;
use crate::infrastructure::platform::printer_api::backend::GraphicsBackendFactory;

/// Default `PrintPort` — creates a fresh native graphics backend per job
/// and delegates rendering to the injected strategy.
pub struct DefaultPrintService {
    render_strategy: Arc<dyn RenderStrategy>,
}

impl DefaultPrintService {
    pub fn new(render_strategy: Arc<dyn RenderStrategy>) -> Self {
        Self { render_strategy }
    }
}

impl PrintPort for DefaultPrintService {
    fn print(
        &self,
        pdf_path: &str,
        printer_name: &str,
        settings: &PrintJobSettings,
    ) -> Result<(), Error> {
        let mut backend = GraphicsBackendFactory::create();
        // Document lifecycle (begin_document / end_document) is managed inside
        // render() on a per-page basis so each label is a separate spooler job.
        // A job is considered done once every page is committed to the OS spooler
        // (queue); we intentionally do NOT wait for physical printing to finish.
        self.render_strategy
            .render(pdf_path, printer_name, settings, &mut *backend)?;
        Ok(())
    }

    fn save_to_path(&self, pdf_path: &Path, output_path: &str) -> Result<(), Error> {
        std::fs::copy(pdf_path, output_path).map_err(|e| InfrastructureError::PrinterError {
            reason: format!("Failed to save PDF to {}: {}", output_path, e),
        })?;
        Ok(())
    }
}
