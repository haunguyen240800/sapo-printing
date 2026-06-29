//! ProcessPrintJobUseCase — executes the full print pipeline for a single job.
//!
//! Pipeline:
//! 1. PENDING → QUEUED
//! 2. Download document (wrapped in RAII temp file via `TempFileManager`)
//! 3. QUEUED → DOWNLOADED
//! 4. Render + send to printer (or copy file for virtual PDF printers)
//! 5. DOWNLOADED → SUBMITTED_TO_QUEUE → PRINTING
//! 6. PRINTING → COMPLETED
//!
//! Each transition persists job state and publishes domain events using the
//! Outbox Pattern (events saved before state update, bus publish is non-fatal).

use std::sync::Arc;

use crate::application::ports::{ConfigProvider, DocumentDownloadService, EventStore, PrintService, TempFileManager};
use crate::application::errors::PipelineError;
use crate::domain::print_job::{PrintJob, PrintJobRepository, PrintJobSettings};
use crate::shared::errors::InfrastructureError;
use crate::shared::event_bus::EventBus;

/// Orchestrates the end-to-end print pipeline for a single popped job.
pub struct ProcessPrintJobUseCase {
    job_repo: Arc<dyn PrintJobRepository>,
    event_store: Arc<dyn EventStore>,
    event_bus: Arc<dyn EventBus>,
    downloader: Arc<dyn DocumentDownloadService>,
    print_service: Arc<dyn PrintService>,
    temp_files: Arc<dyn TempFileManager>,
    config_provider: Arc<dyn ConfigProvider>,
}

impl ProcessPrintJobUseCase {
    pub fn new(
        job_repo: Arc<dyn PrintJobRepository>,
        event_store: Arc<dyn EventStore>,
        event_bus: Arc<dyn EventBus>,
        downloader: Arc<dyn DocumentDownloadService>,
        print_service: Arc<dyn PrintService>,
        temp_files: Arc<dyn TempFileManager>,
        config_provider: Arc<dyn ConfigProvider>,
    ) -> Self {
        Self {
            job_repo,
            event_store,
            event_bus,
            downloader,
            print_service,
            temp_files,
            config_provider,
        }
    }

    pub fn execute(&self, mut job: PrintJob) -> Result<(), PipelineError> {
        // Step 1: PENDING → QUEUED
        job.queue()?;
        if let Err(e) = self.persist_and_publish(&mut job) {
            tracing::error!(
                target = "sapo_printer::application::use_case::process_print_job",
                job_id = %job.id(),
                error = %e,
                "Initial persist failed, marking job as Failed"
            );
            let _ = job.fail(format!("Initial persist failed: {}", e));
            let _ = self.job_repo.update(&job);
            return Err(e);
        }

        // Step 2: Download
        let download_start = std::time::Instant::now();
        let pdf_path = self.downloader.download(job.pdf_url(), job.id())?;
        tracing::info!(
            target = "sapo_printer::metrics",
            job_id = %job.id(),
            step = "download",
            duration_ms = download_start.elapsed().as_millis() as u64,
            "Pipeline step completed"
        );

        let temp_file = self.temp_files.wrap(pdf_path)?;

        job.mark_downloaded()?;
        self.persist_and_publish(&mut job)?;

        // Step 3: Render + send to printer
        let render_start = std::time::Instant::now();
        self.render_or_save(&job, temp_file.as_ref())?;
        tracing::info!(
            target = "sapo_printer::metrics",
            job_id = %job.id(),
            step = "render_and_print",
            duration_ms = render_start.elapsed().as_millis() as u64,
            "Pipeline step completed"
        );

        // Step 4: SUBMITTED_TO_QUEUE → PRINTING → COMPLETED
        job.mark_submitted()?;
        self.persist_and_publish(&mut job)?;

        job.mark_printing()?;
        self.persist_and_publish(&mut job)?;

        job.complete()?;
        self.persist_and_publish(&mut job)?;

        // temp_file dropped here → RAII cleanup
        Ok(())
    }

    fn render_or_save(
        &self,
        job: &PrintJob,
        temp_file: &dyn crate::application::ports::TempFileHandle,
    ) -> Result<(), PipelineError> {
        if let Some(out_path) = job.output_path() {
            tracing::info!(
                target = "sapo_printer::application::use_case::process_print_job",
                job_id = %job.id(),
                out_path = out_path,
                "Virtual PDF printer detected, bypassing spooler"
            );
            self.print_service.save_to_path(temp_file.path(), out_path)?;
            Ok(())
        } else {
            let pdf_path_str = temp_file.path().to_str().ok_or_else(|| {
                PipelineError::Infrastructure(InfrastructureError::ValidationError(
                    "Temp PDF path is not valid UTF-8".to_string(),
                ))
            })?;

            // Load the current print config from file so settings (paper size, margins…)
            // always reflect what the user last saved — the DB does not persist settings.
            let settings: PrintJobSettings = self
                .config_provider
                .load_print_config()
                .map_err(|e| {
                    PipelineError::Infrastructure(InfrastructureError::ValidationError(
                        format!("Failed to load print config: {}", e),
                    ))
                })?
                .as_ref()
                .map(PrintJobSettings::from)
                .unwrap_or_default();

            self.print_service
                .print(pdf_path_str, job.printer_id().as_str(), &settings)?;
            Ok(())
        }
    }

    /// Persist job state and publish drained events (Outbox Pattern).
    fn persist_and_publish(&self, job: &mut PrintJob) -> Result<(), PipelineError> {
        let events = job.drain_events();

        self.event_store
            .save_all(job.id().to_string().as_str(), &events)
            .map_err(|e| PipelineError::Persistence(format!("Failed to save events: {:?}", e)))?;

        self.job_repo
            .update(job)
            .map_err(|e| PipelineError::Persistence(format!("Failed to update job: {:?}", e)))?;

        for event in &events {
            let payload = event.serialize_payload();
            if let Err(e) = self.event_bus.publish(event.event_name(), &payload) {
                tracing::warn!(
                    target = "sapo_printer::application::use_case::process_print_job",
                    event_type = event.event_name(),
                    error = %e,
                    "Failed to publish event (non-fatal)"
                );
            }
        }

        Ok(())
    }
}
