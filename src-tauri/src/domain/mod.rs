// Domain Layer - Core Business Logic (PURE - NO EXTERNAL DEPENDENCIES)
// This layer contains aggregates, entities, value objects, domain events, and repository traits
// CRITICAL: Domain must remain completely independent of all other layers

pub mod common;
pub mod models;
pub mod events;
pub mod repository;
pub mod rules;
// TODO(1-N PrintJob): Uncomment below to use the Document aggregate when implementing 1-N relationship between PrintJob and PDF files
// pub mod document;
