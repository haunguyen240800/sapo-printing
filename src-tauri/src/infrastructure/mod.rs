// Infrastructure Layer - External System Implementations
// This layer implements domain contracts (repositories, services)
//
// Naming convention note:
//   - domain/printer   = Printer aggregate (business rules, entities, value objects)
//   - infrastructure/printer = Print engine (system-level printing APIs)
//   - Use fully qualified paths: crate::domain::printer vs crate::infrastructure::printer
//   - Consider aliasing in downstream code: `use crate::infrastructure::printer as print_engine;`

pub mod config_store;
pub mod database;
pub mod downloader;
pub mod eventbus;
pub mod metrics;
pub mod queue;
pub mod secrets;
pub mod temp_file;
pub mod updater;
pub mod graphics;
pub mod pdfium;
