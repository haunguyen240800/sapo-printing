//! Integration Tests Module Entry Point
//!
//! This module structure allows Cargo to discover and run integration tests.

mod common;

mod secrets {
    #[cfg(target_os = "windows")]
    mod windows_integration_test;

    #[cfg(target_os = "macos")]
    mod macos_integration_test;

    #[cfg(target_os = "linux")]
    mod linux_integration_test;
}

mod cancel_job_integration_test;
mod create_print_job_integration_test;
mod downloader_integration_test;
mod migration_integration_test;
mod native_messaging_integration_test;
mod queue_manager_integration_test;
mod queue_worker_integration_test;
mod renderer_integration_test;
mod repository_integration_test;
mod retry_integration_test;
mod temp_file_integration_test;
mod audit_trail_integration_test;
