use std::sync::Arc;

use crate::application::errors::Error;
use crate::application::ports::event_bus::EventBus;
use crate::application::ports::{DownloadPort, EventStore, PrintPort, TempFilePort};
use crate::domain::print_job::{PrintJob, PrintJobRepository};

pub struct ProcessPrintJobUseCase {
    job_repo: Arc<dyn PrintJobRepository>,
    event_store: Arc<dyn EventStore>,
    event_bus: Arc<dyn EventBus>,
    downloader: Arc<dyn DownloadPort>,
    print_service: Arc<dyn PrintPort>,
    temp_files: Arc<dyn TempFilePort>,
}

impl ProcessPrintJobUseCase {
    pub fn new(
        job_repo: Arc<dyn PrintJobRepository>,
        event_store: Arc<dyn EventStore>,
        event_bus: Arc<dyn EventBus>,
        downloader: Arc<dyn DownloadPort>,
        print_service: Arc<dyn PrintPort>,
        temp_files: Arc<dyn TempFilePort>,
    ) -> Self {
        Self {
            job_repo,
            event_store,
            event_bus,
            downloader,
            print_service,
            temp_files,
        }
    }

    pub fn execute(&self, mut job: PrintJob) -> Result<(), Error> {
        job.begin_processing()?;
        if let Err(e) = self.persist_and_publish(&mut job) {
            tracing::error!(
                target = "sapo_printer::application::use_case::process_print_job",
                job_id = %job.id(),
                error = %e,
                "Initial persist failed, marking job as Failed"
            );
            let _ = job.fail(e.user_message(), e.code().to_string());
            let _ = self.job_repo.update(&job);
            return Err(e);
        }

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

        let render_start = std::time::Instant::now();
        self.render_or_save(&job, temp_file.as_ref())?;
        tracing::info!(
            target = "sapo_printer::metrics",
            job_id = %job.id(),
            step = "render_and_print",
            duration_ms = render_start.elapsed().as_millis() as u64,
            "Pipeline step completed"
        );

        job.mark_submitted()?;
        self.persist_and_publish(&mut job)?;

        job.mark_printing()?;
        self.persist_and_publish(&mut job)?;

        job.complete()?;
        self.persist_and_publish(&mut job)?;

        Ok(())
    }

    fn render_or_save(
        &self,
        job: &PrintJob,
        temp_file: &dyn crate::application::ports::TempFileHandle,
    ) -> Result<(), Error> {
        if let Some(out_path) = job.output_path() {
            tracing::info!(
                target = "sapo_printer::application::use_case::process_print_job",
                job_id = %job.id(),
                out_path = out_path,
                "Virtual PDF printer detected, bypassing spooler"
            );
            self.print_service
                .save_to_path(temp_file.path(), out_path)?;
            Ok(())
        } else {
            let pdf_path_str = temp_file.path().to_str().ok_or_else(|| {
                Error::InvalidInput("Temp PDF path is not valid UTF-8".to_string())
            })?;

            // Dùng snapshot cấu hình đã đóng băng lúc tạo job, KHÔNG nạp lại cấu hình
            // hiện tại, để job giữ đúng khổ giấy/hướng/màu... của thời điểm tạo.
            self.print_service
                .print(pdf_path_str, job.printer_id().as_str(), &job.settings)?;
            Ok(())
        }
    }

    fn persist_and_publish(&self, job: &mut PrintJob) -> Result<(), Error> {
        let events = job.drain_events();

        self.event_store
            .save_all(job.id().to_string().as_str(), &events)
            .map_err(|e| Error::RepositoryError(format!("Failed to save events: {:?}", e)))?;

        self.job_repo
            .update(job)
            .map_err(|e| Error::RepositoryError(format!("Failed to update job: {:?}", e)))?;

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
