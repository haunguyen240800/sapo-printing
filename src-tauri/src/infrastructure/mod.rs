// Infrastructure Layer - External System Implementations
// This layer implements domain contracts (repositories, services)
//
// Infrastructure Layer - External System Implementations
// This layer implements domain contracts (repositories, services)
//
// Naming convention note:
//   - domain/printer   = Printer aggregate (business rules, entities, value objects)
//   - infrastructure/printer = Print engine (system-level printing APIs)
//   - Use fully qualified paths: crate::domain::printer vs crate::infrastructure::printer
//   - Consider aliasing in downstream code: `use crate::infrastructure::printer as print_engine;`

pub mod app_print_config;
pub mod temp_file;

pub mod persistence;
pub mod platform;
pub mod integrations;
pub mod telemetry;
pub mod bus;
