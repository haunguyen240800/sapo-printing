//! Shared test helpers for integration tests.

use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use sapo_printer::infrastructure::database::SqliteEventStore;
use sapo_printer::infrastructure::secrets::SecretManager;
use sapo_printer::shared::errors::InfrastructureError;

/// A mock SecretManager for integration tests that stores secrets in memory.
pub struct MockSecretManager {
    store: Mutex<HashMap<String, String>>,
}

impl MockSecretManager {
    pub fn new() -> Self {
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

/// Create a SqliteEventStore backed by the given connection and a mock SecretManager.
pub fn create_test_event_store(conn: Arc<Mutex<Connection>>) -> Arc<SqliteEventStore> {
    Arc::new(SqliteEventStore::new(
        conn,
        Arc::new(MockSecretManager::new()),
    ))
}
