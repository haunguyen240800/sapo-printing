//! Application Ports — trait abstractions for infrastructure dependencies.
//!
//! Ports define the contracts that use cases depend on.
//! Concrete implementations live in the infrastructure layer and are injected
//! at startup via Dependency Injection.

pub mod event_store;
pub use event_store::{EventStore, StoredEventData};
