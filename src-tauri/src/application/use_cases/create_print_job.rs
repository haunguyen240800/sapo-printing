use std::sync::Arc;

use crate::application::errors::Error;
use crate::application::models::PrintJobCreateRequest;
use crate::application::ports::event_bus::EventBus;
use crate::application::ports::{ConfigPort, EventStore, PrinterAvailability, PrinterPort};
use crate::domain::print_job::{
    PrintJob, PrintJobId, PrintJobRepository, PrintJobSettings, PrinterId,
};

pub struct CreatePrintJobUseCase {
    pub job_repo: Arc<dyn PrintJobRepository>,
    pub event_store: Arc<dyn EventStore>,
    pub event_bus: Arc<dyn EventBus>,
    pub config_provider: Arc<dyn ConfigPort>,
    pub printer_manager: Arc<dyn PrinterPort>,
}

impl CreatePrintJobUseCase {
    pub fn execute(&self, request: PrintJobCreateRequest) -> Result<PrintJobId, Error> {
        tracing::info!(
            target = "sapo_printer::application::use_case::create_print_job",
            url = request.pdf_url,
            "CreatePrintJobUseCase: starting"
        );

        if request.pdf_url.is_empty() {
            return Err(Error::ValidationError {
                reason: "document_url must not be empty".to_string(),
            });
        }

        if request.slip_id.trim().is_empty() {
            return Err(Error::ValidationError {
                reason: "slip_id must not be empty".to_string(),
            });
        }

        let config = self
            .config_provider
            .load_print_config()
            .map_err(|e| Error::ValidationError {
                reason: format!("Failed to load print config: {}", e),
            })?
            .unwrap_or_default();

        let active_printer = config.printer_id.clone();

        if active_printer.is_empty() {
            return Err(Error::PrinterNotAvailable {
                name: "Unknown Printer".to_string(),
            });
        }
        let printer_id = PrinterId::new(active_printer.clone());

        tracing::info!(
            target = "sapo_printer::application::use_case::create_print_job",
            printer = active_printer,
            "Checking printer status"
        );
        match self
            .printer_manager
            .availability(&active_printer)
            .map_err(|e| Error::ValidationError {
                reason: format!("Failed to query printer status: {}", e),
            })? {
            PrinterAvailability::Online => {}
            PrinterAvailability::Offline | PrinterAvailability::Unknown => {
                return Err(Error::PrinterNotAvailable {
                    name: active_printer,
                });
            }
        }

        let settings: PrintJobSettings = (&config).into();

        tracing::info!(
            target = "sapo_printer::application::use_case::create_print_job",
            url = request.pdf_url,
            "Creating job for URL"
        );

        let mut job = PrintJob::new_with_output_path(
            request.slip_id.clone(),
            request.pdf_url.clone(),
            printer_id,
            settings,
            None,
        );
        let events = job.drain_events();

        self.job_repo.save(&job).map_err(|e| {
            tracing::error!(
                target = "sapo_printer::application::use_case::create_print_job",
                error = %e,
                "Failed to save job"
            );
            Error::RepositoryError(e.to_string())
        })?;

        self.event_store
            .save_all(job.id().to_string().as_str(), &events)
            .map_err(|e| {
                tracing::error!(
                    target = "sapo_printer::application::use_case::create_print_job",
                    error = %e,
                    "Failed to save events"
                );
                Error::RepositoryError(e.to_string())
            })?;

        for event in &events {
            if let Err(e) = self
                .event_bus
                .publish(event.event_name(), &event.serialize_payload())
            {
                tracing::warn!(
                    target = "sapo_printer::application::use_case::create_print_job",
                    event_type = %event.event_name(),
                    error = %e,
                    "Event bus publish failed (non-fatal)"
                );
            }
        }

        let job_id = job.id().clone();
        tracing::info!(
            target = "sapo_printer::application::use_case::create_print_job",
            job_id = %job_id,
            "CreatePrintJobUseCase: completed"
        );

        Ok(job_id)
    }
}
