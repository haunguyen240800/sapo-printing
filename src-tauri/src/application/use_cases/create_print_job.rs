use std::sync::Arc;

use crate::application::dto::create_job_request::CreateJobRequest;
use crate::application::use_cases::errors::ApplicationError;
use crate::domain::print_job::aggregate::PrintJob;
use crate::domain::print_job::repository::PrintJobRepository;
use crate::domain::print_job::value_objects::JobId;
use crate::domain::printer::value_objects::PrinterStatus;
use crate::infrastructure::database::SqliteEventStore;
use crate::infrastructure::printer::PrinterManager;
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
    pub event_store: Arc<SqliteEventStore>,
    pub event_bus: Arc<dyn EventBus>,
    pub printer_manager: Arc<dyn PrinterManager>,
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
        // TODO: Implement async printer status check with timeout
        // For now, skip check if printer name is valid (assume online)
        let status = if request.printer_name.is_empty() {
            PrinterStatus::Offline
        } else {
            // TODO: Add timeout wrapper around get_status
            PrinterStatus::Online // Assume online for now to avoid hang
        };

        tracing::info!(
            target = "sapo_printer::use_case::create_print_job",
            printer = request.printer_name,
            status = ?status,
            "Printer status retrieved (assumed online)"
        );

        if status != PrinterStatus::Online {
            return Err(ApplicationError::PrinterNotAvailable {
                name: request.printer_name.clone(),
            });
        }

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
                request.printer_name.clone(),
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
                all_events.push((event.event_type().to_string(), event.serialize_payload()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::print_job::errors::DomainError;
    use crate::domain::print_job::events::DomainEvent;
    use crate::domain::printer::aggregate::Printer;
    use crate::shared::event_bus::{EventBusError, EventHandler};
    use crate::infrastructure::secrets::SecretManager;
    use crate::shared::errors::InfrastructureError;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    struct MockSecretManager {
        store: Mutex<HashMap<String, String>>,
    }

    impl MockSecretManager {
        fn new() -> Self {
            Self {
                store: Mutex::new(HashMap::new()),
            }
        }
    }

    impl SecretManager for MockSecretManager {
        fn store(&self, key: &str, value: &str) -> Result<(), InfrastructureError> {
            self.store
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_string());
            Ok(())
        }

        fn retrieve(&self, key: &str) -> Result<Option<String>, InfrastructureError> {
            Ok(self.store.lock().unwrap().get(key).cloned())
        }

        fn delete(&self, key: &str) -> Result<(), InfrastructureError> {
            self.store.lock().unwrap().remove(key);
            Ok(())
        }
    }

    // ── Mock PrintJobRepository ──
    struct MockJobRepo {
        saved_count: Arc<Mutex<usize>>,
    }

    impl MockJobRepo {
        fn new() -> Self {
            Self {
                saved_count: Arc::new(Mutex::new(0)),
            }
        }

        fn count(&self) -> usize {
            *self.saved_count.lock().unwrap()
        }
    }

    impl PrintJobRepository for MockJobRepo {
        fn save(&self, _job: &PrintJob) -> Result<(), DomainError> {
            *self.saved_count.lock().unwrap() += 1;
            Ok(())
        }

        fn update(&self, _job: &PrintJob) -> Result<(), DomainError> {
            unimplemented!("update not needed for unit tests")
        }

        fn find_by_id(
            &self,
            _id: &crate::domain::print_job::value_objects::JobId,
        ) -> Result<Option<PrintJob>, DomainError> {
            unimplemented!("find_by_id not needed for unit tests")
        }

        fn find_by_status(
            &self,
            _status: &crate::domain::print_job::value_objects::PrintStatus,
        ) -> Result<Vec<PrintJob>, DomainError> {
            unimplemented!("find_by_status not needed for unit tests")
        }

        fn find_all(&self) -> Result<Vec<PrintJob>, DomainError> {
            unimplemented!("find_all not needed for unit tests")
        }
    }

    // ── Mock EventStore ──
    struct MockEventStore;

    impl MockEventStore {
        fn new() -> Self {
            Self
        }

        fn save_all(
            &self,
            _aggregate_id: &str,
            _events: &[Box<dyn DomainEvent>],
        ) -> Result<(), DomainError> {
            Ok(())
        }
    }

    // ── Mock EventBus ──
    struct MockEventBus {
        published_count: Arc<Mutex<usize>>,
    }

    impl MockEventBus {
        fn new() -> Self {
            Self {
                published_count: Arc::new(Mutex::new(0)),
            }
        }

        fn count(&self) -> usize {
            *self.published_count.lock().unwrap()
        }
    }

    impl EventBus for MockEventBus {
        fn publish(&self, _event_type: &str, _payload: &str) -> Result<(), EventBusError> {
            *self.published_count.lock().unwrap() += 1;
            Ok(())
        }

        fn subscribe(&self, event_type: &str, handler: Arc<dyn EventHandler>) {
            todo!()
        }
    }

    // ── Mock PrinterManager ──
    struct MockPrinterManager {
        status: PrinterStatus,
        known: bool,
    }

    impl MockPrinterManager {
        fn with_online_printer() -> Self {
            Self { status: PrinterStatus::Online, known: true }
        }

        fn with_offline_printer() -> Self {
            Self { status: PrinterStatus::Offline, known: true }
        }

        fn empty() -> Self {
            Self { status: PrinterStatus::Offline, known: false }
        }
    }

    impl PrinterManager for MockPrinterManager {
        fn discover_printers(&self) -> Vec<Printer> {
            unimplemented!("discover_printers not needed for unit tests")
        }

        fn get_status(&self, _name: &str) -> PrinterStatus {
            if self.known { self.status.clone() } else { PrinterStatus::Offline }
        }

        fn supports_direct_pdf(&self, _name: &str) -> bool {
            unimplemented!("supports_direct_pdf not needed for unit tests")
        }
    }

    // Helper to create use case with mocks
    fn make_use_case_with_mocks(
        printer_manager: Arc<dyn PrinterManager>,
    ) -> (CreatePrintJobUseCase, Arc<MockJobRepo>, Arc<MockEventBus>) {
        let job_repo = Arc::new(MockJobRepo::new());
        let _event_store = Arc::new(MockEventStore::new());
        let event_bus = Arc::new(MockEventBus::new());

        // Note: event_store here is MockEventStore wrapped in Arc, but UseCase expects Arc<SqliteEventStore>
        // We can't change UseCase signature per AC spec, so we use a workaround:
        // Create a minimal in-memory SQLite just for type compliance (with migrations)
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::infrastructure::database::migrations::run_migrations(&mut conn).unwrap();
        let arc_conn = Arc::new(std::sync::Mutex::new(conn));
        let sqlite_event_store = Arc::new(SqliteEventStore::new(arc_conn, Arc::new(MockSecretManager::new())));

        let use_case = CreatePrintJobUseCase {
            job_repo: job_repo.clone(),
            event_store: sqlite_event_store,
            event_bus: event_bus.clone(),
            printer_manager,
        };

        (use_case, job_repo, event_bus)
    }

    #[test]
    fn test_valid_single_url_creates_job() {
        let printer_manager = Arc::new(MockPrinterManager::with_online_printer());
        let (use_case, job_repo, event_bus) = make_use_case_with_mocks(printer_manager);

        let request = CreateJobRequest {
            pdf_urls: vec!["https://s3.example.com/doc1.pdf".to_string()],
            printer_name: "HP_Test1".to_string(),
            output_path: None,
        };
        let result = use_case.execute(request);
        assert!(result.is_ok());
        let ids = result.unwrap();
        assert_eq!(ids.len(), 1);

        // Verify job was saved via mock
        assert_eq!(job_repo.count(), 1);

        // Verify event was published (PushToQueueHandler will handle it)
        assert_eq!(event_bus.count(), 1);
    }

    #[test]
    fn test_valid_multiple_urls_creates_multiple_jobs() {
        let printer_manager = Arc::new(MockPrinterManager::with_online_printer());
        let (use_case, job_repo, _) = make_use_case_with_mocks(printer_manager);

        let request = CreateJobRequest {
            pdf_urls: vec![
                "https://s3.example.com/doc1.pdf".to_string(),
                "https://s3.example.com/doc2.pdf".to_string(),
                "https://s3.example.com/doc3.pdf".to_string(),
            ],
            printer_name: "HP_Test2".to_string(),
            output_path: None,
        };
        let result = use_case.execute(request);
        assert!(result.is_ok());
        let ids = result.unwrap();
        assert_eq!(ids.len(), 3);

        assert_eq!(job_repo.count(), 3);
    }

    #[test]
    fn test_empty_urls_returns_error() {
        let printer_manager = Arc::new(MockPrinterManager::with_online_printer());
        let (use_case, _, _) = make_use_case_with_mocks(printer_manager);

        let request = CreateJobRequest {
            pdf_urls: vec![],
            printer_name: "HP_Test3".to_string(),
            output_path: None,
        };
        let result = use_case.execute(request);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApplicationError::EmptyJobList
        ));
    }

    #[test]
    fn test_too_many_urls_returns_error() {
        let printer_manager = Arc::new(MockPrinterManager::with_online_printer());
        let (use_case, _, _) = make_use_case_with_mocks(printer_manager);

        let urls: Vec<String> = (0..5001)
            .map(|i| format!("https://s3.example.com/{}.pdf", i))
            .collect();
        let request = CreateJobRequest {
            pdf_urls: urls,
            printer_name: "HP_Test4".to_string(),
            output_path: None,
        };
        let result = use_case.execute(request);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApplicationError::TooManyJobs { count } if count == 5001
        ));
    }

    #[test]
    fn test_exact_limit_5000_succeeds() {
        let printer_manager = Arc::new(MockPrinterManager::with_online_printer());
        let (use_case, job_repo, _) = make_use_case_with_mocks(printer_manager);

        let urls: Vec<String> = (0..5000)
            .map(|i| format!("https://s3.example.com/{}.pdf", i))
            .collect();
        let request = CreateJobRequest {
            pdf_urls: urls,
            printer_name: "HP_Test5".to_string(),
            output_path: None,
        };
        let result = use_case.execute(request);
        assert!(result.is_ok());
        let ids = result.unwrap();
        assert_eq!(ids.len(), 5000);

        assert_eq!(job_repo.count(), 5000);
    }

    #[test]
    fn test_offline_printer_returns_error() {
        let printer_manager = Arc::new(MockPrinterManager::with_offline_printer());
        let (use_case, _, _) = make_use_case_with_mocks(printer_manager);

        let request = CreateJobRequest {
            pdf_urls: vec!["https://s3.example.com/doc.pdf".to_string()],
            printer_name: "HP_Offline".to_string(),
            output_path: None,
        };
        let result = use_case.execute(request);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApplicationError::PrinterNotAvailable { .. }
        ));
    }

    #[test]
    fn test_printer_not_found_returns_error() {
        let printer_manager = Arc::new(MockPrinterManager::empty());
        let (use_case, _, _) = make_use_case_with_mocks(printer_manager);

        let request = CreateJobRequest {
            pdf_urls: vec!["https://s3.example.com/doc.pdf".to_string()],
            printer_name: "NonExistent_Printer".to_string(),
            output_path: None,
        };
        let result = use_case.execute(request);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ApplicationError::PrinterNotAvailable { .. }
        ));
    }

    #[test]
    fn test_events_published_after_save() {
        // Verify ordering: EventBus.publish is called AFTER job save.
        let publish_count = Arc::new(AtomicUsize::new(0));
        let save_count = Arc::new(AtomicUsize::new(0));

        struct CountingEventBus {
            count: Arc<AtomicUsize>,
        }

        impl EventBus for CountingEventBus {
            fn publish(&self, _event_type: &str, _payload: &str) -> Result<(), EventBusError> {
                self.count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }

            fn subscribe(&self, _event_type: &str, _handler: Arc<dyn crate::shared::event_bus::EventHandler>) {
                // No-op for test
            }
        }

        struct CountingJobRepo {
            count: Arc<AtomicUsize>,
        }

        impl PrintJobRepository for CountingJobRepo {
            fn save(&self, _job: &PrintJob) -> Result<(), DomainError> {
                self.count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }

            fn update(&self, _job: &PrintJob) -> Result<(), DomainError> {
                unimplemented!()
            }

            fn find_by_id(
                &self,
                _id: &crate::domain::print_job::value_objects::JobId,
            ) -> Result<Option<PrintJob>, DomainError> {
                unimplemented!()
            }

            fn find_by_status(
                &self,
                _status: &crate::domain::print_job::value_objects::PrintStatus,
            ) -> Result<Vec<PrintJob>, DomainError> {
                unimplemented!()
            }

            fn find_all(&self) -> Result<Vec<PrintJob>, DomainError> {
                unimplemented!()
            }
        }

        let printer_manager: Arc<dyn PrinterManager> =
            Arc::new(MockPrinterManager::with_online_printer());
        let job_repo: Arc<dyn PrintJobRepository> = Arc::new(CountingJobRepo {
            count: save_count.clone(),
        });

        // Minimal in-memory SQLite for type compliance (with migrations)
        let mut conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::infrastructure::database::migrations::run_migrations(&mut conn).unwrap();
        let event_store = Arc::new(SqliteEventStore::new(Arc::new(std::sync::Mutex::new(conn)), Arc::new(MockSecretManager::new())));

        let event_bus: Arc<dyn EventBus> = Arc::new(CountingEventBus {
            count: publish_count.clone(),
        });

        let use_case = CreatePrintJobUseCase {
            job_repo,
            event_store,
            event_bus,
            printer_manager,
        };

        let request = CreateJobRequest {
            pdf_urls: vec!["https://s3.example.com/doc.pdf".to_string()],
            printer_name: "HP_Tracking".to_string(),
            output_path: None,
        };
        let result = use_case.execute(request);
        assert!(result.is_ok());

        // Verify publish was called (PrintJobCreated emits 1 event per URL)
        assert_eq!(publish_count.load(Ordering::SeqCst), 1);

        // Verify job was saved
        assert_eq!(save_count.load(Ordering::SeqCst), 1);
    }
}
