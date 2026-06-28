//! Application layer errors.
//!
//! - `ApplicationError` — surfaced by use cases (validation, repository, domain wrap).
//! - `PipelineError`    — surfaced by `ProcessPrintJobUseCase` (retryable classification).

pub mod application_error;
pub mod pipeline_error;

pub use application_error::ApplicationError;
pub use pipeline_error::PipelineError;
