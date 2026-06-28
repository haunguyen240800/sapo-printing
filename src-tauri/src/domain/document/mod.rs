// Document Domain — Aggregate, Value Objects, Domain Errors

pub mod aggregate;
pub mod errors;
pub mod value_objects;

pub use aggregate::Document;
pub use errors::DocumentDomainError;
pub use value_objects::{DocumentId, DocumentLocation, DocumentType};
