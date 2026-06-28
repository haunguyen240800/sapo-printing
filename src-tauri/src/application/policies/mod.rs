//! Application policies — pure decision logic (no infrastructure dependencies).

pub mod retry_policy;

pub use retry_policy::{calculate_backoff_delay, is_retryable};
