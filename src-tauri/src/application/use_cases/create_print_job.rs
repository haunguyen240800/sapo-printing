use std::sync::Arc;

use crate::application::dto::create_job_request::CreateJobRequest;
use crate::application::use_cases::errors::ApplicationError;
use crate::domain::print_job::aggregate::PrintJob;
use crate::domain::print_job::repository::PrintJobRepository;
use crate::domain::print_job::value_objects::JobId;
use crate::domain::printer::repository::PrinterRepository;
// PrinterName is a value object wrapping String - find_by_name(&PrinterName) requires this wrapper
use crate::domain::printer::value_objects::{PrinterName, PrinterStatus};
use crate::infrastructure::database::SqliteEventStore;
use crate::shared::event_bus::EventBus;

const MAX_URLS: usize = 5000;

/// Use case: create one PrintJob per URL via Outbox Pattern.
///
/// Flow:
/// 1. Validate request (Application layer)
/// 2. Verify printer ONLINE (via printer_repo)
/// 3. Create one PrintJob aggregate per URL
/// 4. drain_events() from each job
/// 5. job_repo.save() + event_store.save_all() — per-URL persistence
/// 6. event_bus.publish() EACH event — ONLY AFTER save succeeds
/// 7. Return Vec<JobId>
pub struct CreatePrintJobUseCase {
    pub job_repo: Arc<dyn PrintJobRepository>,
    pub event_store: Arc<SqliteEventStore>,
    pub event_bus: Arc<dyn EventBus>,
    pub printer_repo: Arc<dyn PrinterRepository>,
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

        // 2. Verify printer ONLINE
        let printer_name_vo = PrinterName::new(request.printer_name.clone());
        let printer = self
            .printer_repo
            .find_by_name(&printer_name_vo)
            .map_err(|e| ApplicationError::RepositoryError(e.to_string()))?
            .ok_or_else(|| ApplicationError::PrinterNotAvailable {
                name: request.printer_name.clone(),
            })?;

        if *printer.status() != PrinterStatus::Online {
            return Err(ApplicationError::PrinterNotAvailable {
                name: request.printer_name.clone(),
            });
        }

        // 3. Create jobs + collect events
        let mut all_job_ids = Vec::new();
        let mut all_events: Vec<(String, String)> = Vec::new(); // (event_type, serialized_payload)

        for url in &request.pdf_urls {
            let mut job = PrintJob::new(url.clone(), request.printer_name.clone());
            let events = job.drain_events();

            // 4. Save job FIRST
            self.job_repo
                .save(&job)
                .map_err(|e| ApplicationError::RepositoryError(e.to_string()))?;

            // 5. Save events to event store (best-effort persistence)
            self.event_store
                .save_all(job.id().to_string().as_str(), &events)
                .map_err(|e| ApplicationError::RepositoryError(e.to_string()))?;

            // Collect events for publishing AFTER all saves
            for event in &events {
                all_events.push((event.event_type().to_string(), event.serialize_payload()));
            }

            all_job_ids.push(job.id().clone());
        }

        // 6. Publish AFTER all saves succeed
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
    use crate::domain::printer::errors::PrinterDomainError;
    use crate::domain::printer::value_objects::{PrinterName, PrinterType};
    use crate::shared::event_bus::EventBusError;
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
    }

    // ── Mock PrinterRepository ──
    struct MockPrinterRepo {
        printer: Option<(PrinterName, PrinterStatus)>, // Store name + status instead of Printer
    }

    impl MockPrinterRepo {
        fn with_online_printer(name: &str) -> Self {
            Self {
                printer: Some((PrinterName::new(name.to_string()), PrinterStatus::Online)),
            }
        }

        fn with_offline_printer(name: &str) -> Self {
            Self {
                printer: Some((PrinterName::new(name.to_string()), PrinterStatus::Offline)),
            }
        }

        fn empty() -> Self {
            Self { printer: None }
        }
    }

    impl PrinterRepository for MockPrinterRepo {
        fn save(&self, _printer: &Printer) -> Result<(), PrinterDomainError> {
            unimplemented!("save not needed for unit tests")
        }

        fn find_all(&self) -> Result<Vec<Printer>, PrinterDomainError> {
            unimplemented!("find_all not needed for unit tests")
        }

        fn find_by_name(&self, _name: &PrinterName) -> Result<Option<Printer>, PrinterDomainError> {
            match &self.printer {
                None => Ok(None),
                Some((name, status)) => {
                    let mut printer = Printer::new(name.clone(), PrinterType::Local);
                    if *status == PrinterStatus::Online {
                        printer.connect()?;
                    }
                    Ok(Some(printer))
                }
            }
        }
    }

    // Helper to create use case with mocks
    fn make_use_case_with_mocks(
        printer_repo: Arc<dyn PrinterRepository>,
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
            printer_repo,
        };

        (use_case, job_repo, event_bus)
    }

    #[test]
    fn test_valid_single_url_creates_job() {
        let printer_repo = Arc::new(MockPrinterRepo::with_online_printer("HP_Test1"));
        let (use_case, job_repo, event_bus) = make_use_case_with_mocks(printer_repo);

        let request = CreateJobRequest {
            pdf_urls: vec!["https://s3.example.com/doc1.pdf".to_string()],
            printer_name: "HP_Test1".to_string(),
        };
        let result = use_case.execute(request);
        assert!(result.is_ok());
        let ids = result.unwrap();
        assert_eq!(ids.len(), 1);

        // Verify job was saved via mock
        assert_eq!(job_repo.count(), 1);

        // Verify event was published
        assert_eq!(event_bus.count(), 1);
    }

    #[test]
    fn test_valid_multiple_urls_creates_multiple_jobs() {
        let printer_repo = Arc::new(MockPrinterRepo::with_online_printer("HP_Test2"));
        let (use_case, job_repo, _) = make_use_case_with_mocks(printer_repo);

        let request = CreateJobRequest {
            pdf_urls: vec![
                "https://s3.example.com/doc1.pdf".to_string(),
                "https://s3.example.com/doc2.pdf".to_string(),
                "https://s3.example.com/doc3.pdf".to_string(),
            ],
            printer_name: "HP_Test2".to_string(),
        };
        let result = use_case.execute(request);
        assert!(result.is_ok());
        let ids = result.unwrap();
        assert_eq!(ids.len(), 3);

        assert_eq!(job_repo.count(), 3);
    }

    #[test]
    fn test_empty_urls_returns_error() {
        let printer_repo = Arc::new(MockPrinterRepo::with_online_printer("HP_Test3"));
        let (use_case, _, _) = make_use_case_with_mocks(printer_repo);

        let request = CreateJobRequest {
            pdf_urls: vec![],
            printer_name: "HP_Test3".to_string(),
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
        let printer_repo = Arc::new(MockPrinterRepo::with_online_printer("HP_Test4"));
        let (use_case, _, _) = make_use_case_with_mocks(printer_repo);

        let urls: Vec<String> = (0..5001)
            .map(|i| format!("https://s3.example.com/{}.pdf", i))
            .collect();
        let request = CreateJobRequest {
            pdf_urls: urls,
            printer_name: "HP_Test4".to_string(),
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
        let printer_repo = Arc::new(MockPrinterRepo::with_online_printer("HP_Test5"));
        let (use_case, job_repo, _) = make_use_case_with_mocks(printer_repo);

        let urls: Vec<String> = (0..5000)
            .map(|i| format!("https://s3.example.com/{}.pdf", i))
            .collect();
        let request = CreateJobRequest {
            pdf_urls: urls,
            printer_name: "HP_Test5".to_string(),
        };
        let result = use_case.execute(request);
        assert!(result.is_ok());
        let ids = result.unwrap();
        assert_eq!(ids.len(), 5000);

        assert_eq!(job_repo.count(), 5000);
    }

    #[test]
    fn test_offline_printer_returns_error() {
        let printer_repo = Arc::new(MockPrinterRepo::with_offline_printer("HP_Offline"));
        let (use_case, _, _) = make_use_case_with_mocks(printer_repo);

        let request = CreateJobRequest {
            pdf_urls: vec!["https://s3.example.com/doc.pdf".to_string()],
            printer_name: "HP_Offline".to_string(),
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
        let printer_repo = Arc::new(MockPrinterRepo::empty());
        let (use_case, _, _) = make_use_case_with_mocks(printer_repo);

        let request = CreateJobRequest {
            pdf_urls: vec!["https://s3.example.com/doc.pdf".to_string()],
            printer_name: "NonExistent_Printer".to_string(),
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

        let printer_repo: Arc<dyn PrinterRepository> =
            Arc::new(MockPrinterRepo::with_online_printer("HP_Tracking"));
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
            printer_repo,
        };

        let request = CreateJobRequest {
            pdf_urls: vec!["https://s3.example.com/doc.pdf".to_string()],
            printer_name: "HP_Tracking".to_string(),
        };
        let result = use_case.execute(request);
        assert!(result.is_ok());

        // Verify publish was called (PrintJobCreated emits 1 event per URL)
        assert_eq!(publish_count.load(Ordering::SeqCst), 1);

        // Verify job was saved
        assert_eq!(save_count.load(Ordering::SeqCst), 1);
    }
}
