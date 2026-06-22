// Infrastructure-level error type. Domain and Application errors live in their
// own layers (`domain::*::errors`, `application::errors`).
pub mod infrastructure_error;

pub use infrastructure_error::InfrastructureError;
