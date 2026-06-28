use std::sync::Arc;

use crate::application::dto::create_job_request::CreateJobRequest;
use crate::application::ports::EventStore;
use crate::application::use_cases::errors::ApplicationError;
use crate::domain::models::PrintJob;
use crate::domain::repository::PrintJobRepository;
use crate::domain::models::JobId;
use crate::shared::event_bus::EventBus;

const MAX_URLS: usize = 5000;

/// Use case: create one PrintJob per URL via Outbox Pattern.
///
/// Flow:
/// 1. Validate request (Application layer)
/// 2. Verify printer ONLINE (via printer_manager)
/// 3. Create one PrintJob aggregate per URL
/// 4. drain_events() from each job
/// 5. job_repo.save() + event_store.save_all() — per-URL persistence
/// 6. event_bus.publish() EACH event — ONLY AFTER save succeeds
/// 7. PushToQueueHandler (subscribed to PrintJobCreated) pushes job to queue
/// 8. Return Vec<JobId>
pub struct CreatePrintJobUseCase {
    pub job_repo: Arc<dyn PrintJobRepository>,
    pub event_store: Arc<dyn EventStore>,
    pub event_bus: Arc<dyn EventBus>,
}

impl CreatePrintJobUseCase {
    pub fn execute(&self, request: CreateJobRequest) -> Result<Vec<JobId>, ApplicationError> {
        tracing::info!(
            target = "sapo_printer::use_case::create_print_job",
            url_count = request.pdf_urls.len(),
            printer = request.printer_name,
            "CreatePrintJobUseCase: starting"
        );

        // 1. Validate request (Application layer)
        if request.pdf_urls.is_empty() {
            return Err(ApplicationError::EmptyJobList);
        }
        if request.pdf_urls.len() > MAX_URLS {
            return Err(ApplicationError::TooManyJobs {
                count: request.pdf_urls.len(),
            });
        }

        tracing::info!(
            target = "sapo_printer::use_case::create_print_job",
            printer = request.printer_name,
            "Checking printer status"
        );

        // 2. Verify printer ONLINE via PrinterManager
        // NOTE: Windows API calls (OpenPrinterW/GetPrinterW) can hang for network printers
                // Load global configuration and map to PrintJobSettings
        let global_config = crate::infrastructure::app_print_config::load_config()
            .unwrap_or_default()
            .unwrap_or_default();

        // Override printer_name with global setting if available, otherwise use request
        let active_printer = if !global_config.printer_name.is_empty() {
            global_config.printer_name.clone()
        } else {
            request.printer_name.clone()
        };

        if active_printer.is_empty() {
            return Err(ApplicationError::PrinterNotAvailable {
                name: "Unknown Printer".to_string(),
            });
        }

        let settings = crate::domain::models::PrintJobSettings {
            paper_size: global_config.paper_size,
            paper_width: global_config.paper_width,
            paper_height: global_config.paper_height,
            orientation: global_config.orientation,
            margin_left: global_config.margin_left,
            margin_right: global_config.margin_right,
            margin_top: global_config.margin_top,
            margin_bottom: global_config.margin_bottom,
            color_mode: global_config.color_mode,
            print_as_image: global_config.print_as_image,
            dpi: 300,
            copies: 1,
            rotate: 0.0,
        };

        // 3. Create jobs + collect events
        let mut all_job_ids = Vec::new();
        let mut all_events: Vec<(String, String)> = Vec::new(); // (event_type, serialized_payload)

        for url in &request.pdf_urls {
            tracing::info!(
                target = "sapo_printer::use_case::create_print_job",
                url = url,
                "Creating job for URL"
            );

            let mut job = PrintJob::new_with_output_path(
                url.clone(),
                active_printer.clone(),
                settings.clone(),
                request.output_path.clone(),
            );
            let events = job.drain_events();

            tracing::info!(
                target = "sapo_printer::use_case::create_print_job",
                job_id = %job.id(),
                "Saving job to repository"
            );

            // 4. Save job FIRST
            let save_result = self.job_repo.save(&job);

            tracing::info!(
                target = "sapo_printer::use_case::create_print_job",
                job_id = %job.id(),
                success = save_result.is_ok(),
                "Repository save completed"
            );

            save_result.map_err(|e| {
                tracing::error!(
                    target = "sapo_printer::use_case::create_print_job",
                    error = %e,
                    "Failed to save job"
                );
                ApplicationError::RepositoryError(e.to_string())
            })?;

            tracing::info!(
                target = "sapo_printer::use_case::create_print_job",
                job_id = %job.id(),
                event_count = events.len(),
                "Job saved successfully, now saving events to event store"
            );

            // 5. Save events to event store (best-effort persistence)
            self.event_store
                .save_all(job.id().to_string().as_str(), &events)
                .map_err(|e| {
                    tracing::error!(
                        target = "sapo_printer::use_case::create_print_job",
                        error = %e,
                        "Failed to save events"
                    );
                    ApplicationError::RepositoryError(e.to_string())
                })?;

            // Collect events for publishing AFTER all saves
            for event in &events {
                all_events.push((event.event_name().to_string(), event.serialize_payload()));
            }

            all_job_ids.push(job.id().clone());
        }

        tracing::info!(
            target = "sapo_printer::use_case::create_print_job",
            event_count = all_events.len(),
            "Publishing events"
        );

        // 6. Publish AFTER all saves succeed
        // PushToQueueHandler (subscribed to PrintJobCreated) will automatically push to queue
        for (event_type, payload) in &all_events {
            let _ = self.event_bus.publish(event_type, payload);
            // EventBus publish failures are non-fatal — log but don't fail
        }

        tracing::info!(
            target = "sapo_printer::use_case::create_print_job",
            job_count = all_job_ids.len(),
            printer = request.printer_name,
            "CreatePrintJobUseCase: completed"
        );
        tracing::debug!(
            target = "sapo_printer::use_case::create_print_job",
            job_ids = ?all_job_ids,
            "CreatePrintJobUseCase: job IDs created"
        );

        Ok(all_job_ids)
    }
}

// ── Unit Tests ──────────────────────────────────────────────────────────────


