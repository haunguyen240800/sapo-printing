pub mod collector;

use std::fmt;

pub use collector::MetricsCollector;

#[derive(Debug)]
pub enum MetricsError {
    DatabaseError(String),
    QueueError(String),
}

impl fmt::Display for MetricsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DatabaseError(msg) => write!(f, "Metrics database error: {}", msg),
            Self::QueueError(msg) => write!(f, "Metrics queue error: {}", msg),
        }
    }
}

impl std::error::Error for MetricsError {}
