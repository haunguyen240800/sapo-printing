use std::sync::Arc;
use crate::domain::print_job::PrintJob;
use crate::domain::common::aggregate::AggregateRoot;
use crate::application::services::layout_engine::LayoutEngine;
use crate::infrastructure::graphics::backend::GraphicsBackendFactory;
use crate::infrastructure::pdfium::bitmap_strategy::BitmapRenderStrategy;
use crate::infrastructure::pdfium::native_strategy::NativePdfRenderStrategy;
use crate::infrastructure::pdfium::renderer::RenderStrategy;

pub trait IJobRepository: Send + Sync {
    fn save(&self, job: &PrintJob) -> Result<(), String>;
}

pub trait IEventBus: Send + Sync {
    fn publish(&self, event: &dyn crate::domain::common::aggregate::DomainEvent) -> Result<(), String>;
}

pub trait IPdfDownloader: Send + Sync {
    fn download(&self, url: &str) -> Result<String, String>;
}

pub trait PrintService {
    fn execute(&self, job: PrintJob) -> Result<(), String>;
}

pub struct DefaultPrintService {
    job_repo: Arc<dyn IJobRepository>,
    event_bus: Arc<dyn IEventBus>,
    downloader: Arc<dyn IPdfDownloader>,
}

impl DefaultPrintService {
    pub fn new(
        job_repo: Arc<dyn IJobRepository>,
        event_bus: Arc<dyn IEventBus>,
        downloader: Arc<dyn IPdfDownloader>,
    ) -> Self {
        Self {
            job_repo,
            event_bus,
            downloader,
        }
    }

    fn persist_and_publish(&self, job: &mut PrintJob) -> Result<(), String> {
        self.job_repo.save(job)?;
        for event in job.domain_events() {
            self.event_bus.publish(event.as_ref())?;
        }
        job.clear_domain_events();
        Ok(())
    }
}

// Dummy PDF size getter for compilation
fn get_pdf_size(_path: &str) -> (f32, f32) {
    (100.0, 150.0)
}

impl PrintService for DefaultPrintService {
    fn execute(&self, mut job: PrintJob) -> Result<(), String> {
        let pdf_path = self.downloader.download(job.pdf_url())?;
        
        job.mark_downloaded().map_err(|e| format!("{:?}", e))?;
        self.persist_and_publish(&mut job)?;

        let (pdf_w, pdf_h) = get_pdf_size(&pdf_path); 
        let transform = LayoutEngine::calculate(&job.settings, pdf_w, pdf_h);

        job.mark_printing().map_err(|e| format!("{:?}", e))?;
        self.persist_and_publish(&mut job)?;
        
        let mut backend = GraphicsBackendFactory::create();
        backend.begin_document(job.printer_name(), "Sapo Order")?;
        
        let strategy: Box<dyn RenderStrategy> = if job.settings.binary {
            Box::new(BitmapRenderStrategy::new())
        } else {
            Box::new(NativePdfRenderStrategy::new())
        };
        
        strategy.render(&pdf_path, &transform, &job.settings, &mut *backend);
        
        backend.end_document();

        job.complete().map_err(|e| format!("{:?}", e))?;
        self.persist_and_publish(&mut job)?;
        
        Ok(())
    }
}
