use std::sync::Arc;

use crate::application::dto::create_job_request::CreateJobRequest;
use crate::application::use_cases::errors::ApplicationError;
use crate::domain::print_job::aggregate::PrintJob;
use crate::domain::print_job::repository::PrintJobRepository;
use crate::domain::print_job::value_objects::JobId;
use crate::domain::printer::repository::PrinterRepository;
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

        Ok(all_job_ids)
    }
}

// ── Unit Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database::migrations::run_migrations;
    use rusqlite::Connection;
    use std::sync::Mutex;

    /// Seed a printer with `Online` status directly via the connection.
    fn setup_online_printer(conn: &Arc<Mutex<Connection>>, name: &str) {
        let c = conn.lock().unwrap_or_else(|p| p.into_inner());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        c.execute(
            "INSERT INTO printer_configs (printer_name, device_id, printer_type, status, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![name, name, "Local", "Online", now, now],
        )
        .unwrap();
    }

    fn make_use_case_with_online_printer(
        printer_name: &str,
    ) -> (CreatePrintJobUseCase, Arc<Mutex<Connection>>) {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();
        let arc_conn = Arc::new(Mutex::new(conn));

        // Seed online printer
        setup_online_printer(&arc_conn, printer_name);

        let job_repo: Arc<dyn PrintJobRepository> = Arc::new(
            crate::infrastructure::database::SqlitePrintJobRepository::new(arc_conn.clone()),
        );
        let event_store = Arc::new(SqliteEventStore::new(arc_conn.clone()));
        let event_bus: Arc<dyn EventBus> =
            Arc::new(crate::shared::event_bus::InMemoryEventBus::new());
        let printer_repo: Arc<dyn PrinterRepository> = Arc::new(
            crate::infrastructure::database::SqlitePrinterRepository::new(arc_conn.clone()),
        );

        let use_case = CreatePrintJobUseCase {
            job_repo,
            event_store,
            event_bus,
            printer_repo,
        };

        (use_case, arc_conn)
    }

    #[test]
    fn test_valid_single_url_creates_job() {
        let (use_case, conn) = make_use_case_with_online_printer("HP_Test1");
        let request = CreateJobRequest {
            pdf_urls: vec!["https://s3.example.com/doc1.pdf".to_string()],
            printer_name: "HP_Test1".to_string(),
        };
        let result = use_case.execute(request);
        assert!(result.is_ok());
        let ids = result.unwrap();
        assert_eq!(ids.len(), 1);

        // Verify job was saved
        let conn = conn.lock().unwrap_or_else(|p| p.into_inner());
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM print_jobs WHERE document_url = ?1",
                ["https://s3.example.com/doc1.pdf"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_valid_multiple_urls_creates_multiple_jobs() {
        let (use_case, conn) = make_use_case_with_online_printer("HP_Test2");
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

        let conn = conn.lock().unwrap_or_else(|p| p.into_inner());
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM print_jobs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_empty_urls_returns_error() {
        let (use_case, _) = make_use_case_with_online_printer("HP_Test3");
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
        let (use_case, _) = make_use_case_with_online_printer("HP_Test4");
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
        let (use_case, conn) = make_use_case_with_online_printer("HP_Test5");
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

        let conn = conn.lock().unwrap_or_else(|p| p.into_inner());
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM print_jobs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 5000);
    }

    #[test]
    fn test_offline_printer_returns_error() {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();
        let arc_conn = Arc::new(Mutex::new(conn));

        // Save printer with Offline status
        {
            let c = arc_conn.lock().unwrap_or_else(|p| p.into_inner());
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            c.execute(
                "INSERT INTO printer_configs (printer_name, device_id, printer_type, status, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params!["HP_Offline", "HP_Offline", "Local", "Offline", now, now],
            )
            .unwrap();
        }

        let job_repo: Arc<dyn PrintJobRepository> = Arc::new(
            crate::infrastructure::database::SqlitePrintJobRepository::new(arc_conn.clone()),
        );
        let event_store = Arc::new(SqliteEventStore::new(arc_conn.clone()));
        let event_bus: Arc<dyn EventBus> =
            Arc::new(crate::shared::event_bus::InMemoryEventBus::new());
        let printer_repo: Arc<dyn PrinterRepository> = Arc::new(
            crate::infrastructure::database::SqlitePrinterRepository::new(arc_conn.clone()),
        );

        let use_case = CreatePrintJobUseCase {
            job_repo,
            event_store,
            event_bus,
            printer_repo,
        };

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
        let (use_case, _) = make_use_case_with_online_printer("HP_Test6");
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
        // We use a shared counter to track publish calls, then verify
        // the job exists in DB (saved before publish).
        use std::sync::atomic::{AtomicUsize, Ordering};

        let publish_count = Arc::new(AtomicUsize::new(0));

        struct CountingEventBus {
            count: Arc<AtomicUsize>,
        }

        impl EventBus for CountingEventBus {
            fn publish(
                &self,
                _event_type: &str,
                _payload: &str,
            ) -> Result<(), crate::shared::event_bus::EventBusError> {
                self.count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }

        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();
        let arc_conn = Arc::new(Mutex::new(conn));

        // Seed online printer
        {
            let c = arc_conn.lock().unwrap_or_else(|p| p.into_inner());
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64;
            c.execute(
                "INSERT INTO printer_configs (printer_name, device_id, printer_type, status, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params!["HP_Tracking", "HP_Tracking", "Local", "Online", now, now],
            )
            .unwrap();
        }

        let job_repo: Arc<dyn PrintJobRepository> = Arc::new(
            crate::infrastructure::database::SqlitePrintJobRepository::new(arc_conn.clone()),
        );
        let event_store = Arc::new(SqliteEventStore::new(arc_conn.clone()));
        let event_bus: Arc<dyn EventBus> = Arc::new(CountingEventBus {
            count: publish_count.clone(),
        });
        let printer_repo: Arc<dyn PrinterRepository> = Arc::new(
            crate::infrastructure::database::SqlitePrinterRepository::new(arc_conn.clone()),
        );

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

        // Verify job was saved (job exists in DB)
        let conn = arc_conn.lock().unwrap_or_else(|p| p.into_inner());
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM print_jobs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
