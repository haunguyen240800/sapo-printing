//! Entities owned by the `PrintJob` aggregate.
//!
//! Entities have identity that persists over time and are mutated only via the
//! aggregate root (`PrintJob`). They are never persisted independently — their
//! lifecycle is bound to the parent aggregate.

pub mod print_task;

pub use print_task::PrintTask;
