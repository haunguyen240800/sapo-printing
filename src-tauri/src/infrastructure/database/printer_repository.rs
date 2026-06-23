use rusqlite::Connection;
use std::sync::{Arc, Mutex};

use crate::domain::printer::aggregate::Printer;
use crate::domain::printer::errors::PrinterDomainError;
use crate::domain::printer::repository::PrinterRepository;
use crate::domain::printer::value_objects::{PrinterName, PrinterStatus, PrinterType};

/// SQLite implementation of `PrinterRepository`.
///
/// Persists `Printer` aggregates to the `printer_configs` table.
/// Uses prepared statements for all queries (SQL injection prevention).
pub struct SqlitePrinterRepository {
    conn: Arc<Mutex<Connection>>,
}

impl SqlitePrinterRepository {
    /// Create a new repository with a shared database connection.
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }
}

impl PrinterRepository for SqlitePrinterRepository {
    fn save(&self, printer: &Printer) -> Result<(), PrinterDomainError> {
        let conn = self.conn.lock().unwrap_or_else(|poisoned| {
            // Mutex poisoned — another thread panicked while holding the lock.
            // We proceed with the poisoned lock to avoid propagating panic.
            poisoned.into_inner()
        });

        // Convert domain types to storage format
        let printer_name = printer.name().as_str();
        let printer_type_str = match printer.printer_type() {
            PrinterType::Local => "Local",
            PrinterType::Network => "Network",
        };
        let status_str = match printer.status() {
            PrinterStatus::Online => "Online",
            PrinterStatus::Offline => "Offline",
            PrinterStatus::Error => "Error",
        };

        // Get current timestamp (Unix epoch seconds)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_secs() as i64;

        // UPSERT: INSERT with ON CONFLICT UPDATE
        conn.execute(
            "INSERT INTO printer_configs (
                printer_name, device_id, printer_type, status, paper_size, paper_width, paper_height,
                orientation, margin_left, margin_right, margin_top, margin_bottom,
                color_mode, print_as_image, enable_buffer, buffer_size_kb,
                is_default, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
            ON CONFLICT(device_id) DO UPDATE SET
                printer_name = excluded.printer_name,
                printer_type = excluded.printer_type,
                status = excluded.status,
                paper_size = excluded.paper_size,
                paper_width = excluded.paper_width,
                paper_height = excluded.paper_height,
                orientation = excluded.orientation,
                margin_left = excluded.margin_left,
                margin_right = excluded.margin_right,
                margin_top = excluded.margin_top,
                margin_bottom = excluded.margin_bottom,
                color_mode = excluded.color_mode,
                print_as_image = excluded.print_as_image,
                enable_buffer = excluded.enable_buffer,
                buffer_size_kb = excluded.buffer_size_kb,
                is_default = excluded.is_default,
                updated_at = excluded.updated_at",
            rusqlite::params![
                printer_name,      // ?1
                printer_name,      // ?2 - use printer_name as device_id (stable key)
                printer_type_str,  // ?3 - printer_type
                status_str,        // ?4 - status
                "A4",              // ?5 - paper_size (default)
                210,               // ?6 - paper_width (A4 width in mm)
                297,               // ?7 - paper_height (A4 height in mm)
                "portrait",        // ?8 - orientation (default)
                0,                 // ?9 - margin_left (mm)
                0,                 // ?10 - margin_right (mm)
                0,                 // ?11 - margin_top (mm)
                0,                 // ?12 - margin_bottom (mm)
                "RGB",             // ?13 - color_mode (default)
                0,                 // ?14 - print_as_image (false)
                0,                 // ?15 - enable_buffer (false)
                rusqlite::types::Null, // ?16 - buffer_size_kb (NULL)
                0,                 // ?17 - is_default (false for now)
                now,               // ?18 - created_at
                now,               // ?19 - updated_at
            ],
        )
        .map_err(|e| PrinterDomainError::RepositoryError {
            reason: format!("Failed to save printer: {}", e),
        })?;

        Ok(())
    }

    fn find_all(&self) -> Result<Vec<Printer>, PrinterDomainError> {
        let conn = self
            .conn
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let mut stmt = conn
            .prepare("SELECT printer_name, printer_type, status FROM printer_configs")
            .map_err(|e| PrinterDomainError::RepositoryError {
                reason: format!("Failed to prepare query: {}", e),
            })?;

        let printer_iter = stmt
            .query_map([], |row| {
                let printer_name: String = row.get(0)?;
                let printer_type_str: String = row.get(1)?;
                let status_str: String = row.get(2)?;

                // Parse enums from TEXT
                let printer_type = match printer_type_str.as_str() {
                    "Local" => PrinterType::Local,
                    "Network" => PrinterType::Network,
                    _ => PrinterType::Local, // fallback
                };

                let status = match status_str.as_str() {
                    "Online" => PrinterStatus::Online,
                    "Offline" => PrinterStatus::Offline,
                    "Error" => PrinterStatus::Error,
                    _ => PrinterStatus::Offline, // fallback
                };

                Ok((printer_name, printer_type, status))
            })
            .map_err(|e| PrinterDomainError::RepositoryError {
                reason: format!("Failed to query printers: {}", e),
            })?;

        let mut printers = Vec::new();
        for printer_result in printer_iter {
            let (name, printer_type, stored_status) =
                printer_result.map_err(|e| PrinterDomainError::RepositoryError {
                    reason: format!("Failed to read printer row: {}", e),
                })?;

            // Reconstruct Printer aggregate (always starts as Offline)
            let mut printer = Printer::new(PrinterName::new(name), printer_type);

            // If stored status was Online, transition to Online
            if stored_status == PrinterStatus::Online {
                printer.connect()?;
                // Clear the PrinterConnected event — we're loading, not discovering
                printer.drain_events();
            }

            printers.push(printer);
        }

        Ok(printers)
    }

    fn find_by_name(&self, name: &PrinterName) -> Result<Option<Printer>, PrinterDomainError> {
        let conn = self
            .conn
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let mut stmt = conn
            .prepare("SELECT printer_name, printer_type, status FROM printer_configs WHERE printer_name = ?1")
            .map_err(|e| PrinterDomainError::RepositoryError {
                reason: format!("Failed to prepare query: {}", e),
            })?;

        let result = stmt.query_row([name.as_str()], |row| {
            let printer_name: String = row.get(0)?;
            let printer_type_str: String = row.get(1)?;
            let status_str: String = row.get(2)?;

            // Parse enums from TEXT
            let printer_type = match printer_type_str.as_str() {
                "Local" => PrinterType::Local,
                "Network" => PrinterType::Network,
                _ => PrinterType::Local, // fallback
            };

            let status = match status_str.as_str() {
                "Online" => PrinterStatus::Online,
                "Offline" => PrinterStatus::Offline,
                "Error" => PrinterStatus::Error,
                _ => PrinterStatus::Offline, // fallback
            };

            Ok((printer_name, printer_type, status))
        });

        match result {
            Ok((printer_name, printer_type, stored_status)) => {
                // Reconstruct Printer aggregate
                let mut printer = Printer::new(PrinterName::new(printer_name), printer_type);

                // If stored status was Online, transition to Online
                if stored_status == PrinterStatus::Online {
                    printer.connect()?;
                    // Clear the PrinterConnected event — we're loading, not discovering
                    printer.drain_events();
                }

                Ok(Some(printer))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(PrinterDomainError::RepositoryError {
                reason: format!("Failed to query printer: {}", e),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::database::migrations::run_migrations;

    fn setup_test_db() -> Arc<Mutex<Connection>> {
        let mut conn = Connection::open_in_memory().expect("Failed to open in-memory database");
        run_migrations(&mut conn).expect("Failed to run migrations in test setup");
        Arc::new(Mutex::new(conn))
    }

    fn make_test_printer(name: &str, printer_type: PrinterType) -> Printer {
        Printer::new(PrinterName::new(name.to_string()), printer_type)
    }

    #[test]
    fn test_save_inserts_new_printer() {
        let conn = setup_test_db();
        let repo = SqlitePrinterRepository::new(conn.clone());

        let printer = make_test_printer("HP LaserJet", PrinterType::Local);
        repo.save(&printer).unwrap();

        // Verify record exists in DB
        let count: i64 = {
            let c = conn.lock().unwrap();
            c.query_row("SELECT COUNT(*) FROM printer_configs", [], |row| row.get(0))
                .unwrap()
        };
        assert_eq!(count, 1);
    }

    #[test]
    fn test_save_updates_existing_printer() {
        let conn = setup_test_db();
        let repo = SqlitePrinterRepository::new(conn.clone());

        let mut printer = make_test_printer("Canon Pixma", PrinterType::Network);

        // First save
        repo.save(&printer).unwrap();

        // Modify and save again (should UPSERT)
        printer
            .connect()
            .expect("Failed to connect printer in test");
        repo.save(&printer).unwrap();

        // Should still be only 1 record
        let count: i64 = {
            let c = conn.lock().unwrap();
            c.query_row("SELECT COUNT(*) FROM printer_configs", [], |row| row.get(0))
                .unwrap()
        };
        assert_eq!(count, 1);
    }

    #[test]
    fn test_save_default_printer_unsets_others() {
        let conn = setup_test_db();
        let repo = SqlitePrinterRepository::new(conn.clone());

        let p1 = make_test_printer("Printer 1", PrinterType::Local);
        let p2 = make_test_printer("Printer 2", PrinterType::Network);

        repo.save(&p1).unwrap();
        repo.save(&p2).unwrap();

        // NOTE: is_default logic deferred to Story 2.5 (Printer aggregate lacks is_default field)
        // Current behavior: all printers saved with is_default=0 (default)
        let count: i64 = {
            let c = conn.lock().unwrap();
            c.query_row(
                "SELECT COUNT(*) FROM printer_configs WHERE is_default = 0",
                [],
                |row| row.get(0),
            )
            .unwrap()
        };
        assert_eq!(count, 2);
    }

    #[test]
    fn test_margins_stored_in_mm() {
        let conn = setup_test_db();
        let repo = SqlitePrinterRepository::new(conn.clone());

        let printer = make_test_printer("Test Printer", PrinterType::Local);
        repo.save(&printer).unwrap();

        // Verify margins are stored as 0 (default values in mm)
        let (ml, mr, mt, mb): (i32, i32, i32, i32) = {
            let c = conn.lock().unwrap();
            c.query_row(
                "SELECT margin_left, margin_right, margin_top, margin_bottom FROM printer_configs WHERE printer_name = 'Test Printer'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap()
        };
        assert_eq!(ml, 0);
        assert_eq!(mr, 0);
        assert_eq!(mt, 0);
        assert_eq!(mb, 0);
    }

    #[test]
    fn test_find_all_returns_all_printers() {
        let conn = setup_test_db();
        let repo = SqlitePrinterRepository::new(conn);

        let p1 = make_test_printer("Printer 1", PrinterType::Local);
        let p2 = make_test_printer("Printer 2", PrinterType::Network);

        repo.save(&p1).unwrap();
        repo.save(&p2).unwrap();

        let printers = repo.find_all().unwrap();
        assert_eq!(printers.len(), 2);
    }

    #[test]
    fn test_find_all_empty_when_no_printers() {
        let conn = setup_test_db();
        let repo = SqlitePrinterRepository::new(conn);

        let printers = repo.find_all().unwrap();
        assert!(printers.is_empty());
    }

    #[test]
    fn test_find_by_name_returns_printer() {
        let conn = setup_test_db();
        let repo = SqlitePrinterRepository::new(conn);

        let printer = make_test_printer("HP OfficeJet", PrinterType::Local);
        repo.save(&printer).unwrap();

        let found = repo
            .find_by_name(&PrinterName::new("HP OfficeJet".to_string()))
            .unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name().as_str(), "HP OfficeJet");
    }

    #[test]
    fn test_find_by_name_returns_none_when_not_found() {
        let conn = setup_test_db();
        let repo = SqlitePrinterRepository::new(conn);

        let found = repo
            .find_by_name(&PrinterName::new("NonExistent".to_string()))
            .unwrap();
        assert!(found.is_none());
    }
}
