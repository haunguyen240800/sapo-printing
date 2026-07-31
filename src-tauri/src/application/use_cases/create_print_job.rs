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

const MAX_URLS: usize = 5000;

/// Use case: create one PrintJob per URL via Outbox Pattern.
///
/// Flow:
/// 1. Validate request (Application layer)
/// 2. Resolve effective config + printer name via `ConfigProvider`
/// 3. Verify printer ONLINE via `PrinterManager`
/// 4. Create one PrintJob aggregate per URL
/// 5. drain_events() from each job
/// 6. job_repo.save() + event_store.save_all() — per-URL persistence
/// 7. event_bus.publish() EACH event — ONLY AFTER save succeeds
/// 8. PushToQueueHandler (subscribed to PrintJobCreated) pushes job to queue
/// 9. Return Vec<PrintJobId>
pub struct CreatePrintJobUseCase {
    pub job_repo: Arc<dyn PrintJobRepository>,
    pub event_store: Arc<dyn EventStore>,
    pub event_bus: Arc<dyn EventBus>,
    pub config_provider: Arc<dyn ConfigProvider>,
    pub printer_manager: Arc<dyn PrinterManager>,
}

impl CreatePrintJobUseCase {
    pub fn execute(&self, request: CreatePrintJobRequest) -> Result<Vec<PrintJobId>, ApplicationError> {
        tracing::info!(
            target = "sapo_printer::application::use_case::create_print_job",
            url_count = request.pdf_urls.len(),
            "CreatePrintJobUseCase: starting"
        );

        // 1. Validate request
        if request.pdf_urls.is_empty() {
            return Err(ApplicationError::EmptyJobList);
        }
        if request.pdf_urls.len() > MAX_URLS {
            return Err(ApplicationError::TooManyJobs {
                count: request.pdf_urls.len(),
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

        // 4. Create jobs + collect events
        let mut all_job_ids = Vec::new();
        let mut all_events: Vec<(String, String)> = Vec::new();

        for url in &request.pdf_urls {
            tracing::info!(
                target = "sapo_printer::application::use_case::create_print_job",
                url = url,
                "Creating job for URL"
            );

            let mut job = PrintJob::new_with_output_path(
                url.clone(),
                printer_id.clone(),
                settings.clone(),
                request.output_path.clone(),
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

            for event in &events {
                all_events.push((event.event_name().to_string(), event.serialize_payload()));
            }

            all_job_ids.push(job.id().clone());
        }

        // 5. Publish AFTER all saves succeed
        for (event_type, payload) in &all_events {
            if let Err(e) = self.event_bus.publish(event_type, payload) {
                tracing::warn!(
                    target = "sapo_printer::application::use_case::create_print_job",
                    event_type = %event_type,
                    error = %e,
                    "Event bus publish failed (non-fatal)"
                );
            }
        }

        tracing::info!(
            target = "sapo_printer::application::use_case::create_print_job",
            job_count = all_job_ids.len(),
            "CreatePrintJobUseCase: completed"
        );

        Ok(all_job_ids)
    }
}

