// Library root - declares 4-layer Clean Architecture modules
// This file is the entry point for the library crate

// Interface Layer - External-facing APIs
pub mod interface;

// Application Layer - Use Cases and Application Services
pub mod application;

// Domain Layer - Core Business Logic (PURE - NO EXTERNAL DEPENDENCIES)
pub mod domain;

// Infrastructure Layer - External System Implementations
pub mod infrastructure;

// Shared Layer - Cross-Cutting Concerns
pub mod shared;
