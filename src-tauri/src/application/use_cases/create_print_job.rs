use std::sync::Arc;

use crate::application::dto::create_print_job_request::CreatePrintJobRequest;
use crate::application::ports::{
    ConfigProvider, EventStore, PrinterAvailability, PrinterManager,
};
use crate::application::errors::ApplicationError;
use crate::domain::print_job::{
    PrintJob, PrintJobId, PrintJobRepository, PrintJobSettings, PrinterId,
};
use crate::shared::event_bus::EventBus;

/// Use case: create one PrintJob for a single PDF URL via Outbox Pattern.
///
/// Flow:
/// 1. Validate request (Application layer)
/// 2. Resolve effective config + printer name via `ConfigProvider`
/// 3. Verify printer ONLINE via `PrinterManager`
/// 4. Create PrintJob aggregate
/// 5. drain_events() from job
/// 6. job_repo.save() + event_store.save_all() — persistence
/// 7. event_bus.publish() EACH event — ONLY AFTER save succeeds
/// 8. PushToQueueHandler (subscribed to PrintJobCreated) pushes job to queue
/// 9. Return PrintJobId
pub struct CreatePrintJobUseCase {
    pub job_repo: Arc<dyn PrintJobRepository>,
    pub event_store: Arc<dyn EventStore>,
    pub event_bus: Arc<dyn EventBus>,
    pub config_provider: Arc<dyn ConfigProvider>,
    pub printer_manager: Arc<dyn PrinterManager>,
}

impl CreatePrintJobUseCase {
    pub fn execute(&self, request: CreatePrintJobRequest) -> Result<PrintJobId, ApplicationError> {
        tracing::info!(
            target = "sapo_printer::application::use_case::create_print_job",
            url = request.pdf_url,
            "CreatePrintJobUseCase: starting"
        );

        // 1. Validate request
        if request.pdf_url.is_empty() {
            return Err(ApplicationError::ValidationError {
                reason: "document_url must not be empty".to_string(),
            });
        }

        // 2. Resolve effective config (global override wins over request)
        let config = self
            .config_provider
            .load_print_config()
            .map_err(|e| ApplicationError::ValidationError {
                reason: format!("Failed to load print config: {}", e),
            })?
            .unwrap_or_default();

        let active_printer = config.printer_id.clone();

        if active_printer.is_empty() {
            return Err(ApplicationError::PrinterNotAvailable {
                name: "Unknown Printer".to_string(),
            });
        }
        let printer_id = PrinterId::new(active_printer.clone());

        // 3. Verify printer ONLINE
        tracing::info!(
            target = "sapo_printer::application::use_case::create_print_job",
            printer = active_printer,
            "Checking printer status"
        );
        match self
            .printer_manager
            .availability(&active_printer)
            .map_err(|e| ApplicationError::ValidationError {
                reason: format!("Failed to query printer status: {}", e),
            })? {
            PrinterAvailability::Online => {}
            PrinterAvailability::Offline | PrinterAvailability::Unknown => {
                return Err(ApplicationError::PrinterNotAvailable {
                    name: active_printer,
                });
            }
        }

        let settings: PrintJobSettings = (&config).into();

        // 4. Create job + collect events
        tracing::info!(
            target = "sapo_printer::application::use_case::create_print_job",
            url = request.pdf_url,
            "Creating job for URL"
        );

        let mut job = PrintJob::new_with_output_path(
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
            ApplicationError::RepositoryError(e.to_string())
        })?;

        self.event_store
            .save_all(job.id().to_string().as_str(), &events)
            .map_err(|e| {
                tracing::error!(
                    target = "sapo_printer::application::use_case::create_print_job",
                    error = %e,
                    "Failed to save events"
                );
                ApplicationError::RepositoryError(e.to_string())
            })?;

        // 5. Publish AFTER save succeeds
        for event in &events {
            if let Err(e) = self.event_bus.publish(event.event_name(), &event.serialize_payload()) {
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

