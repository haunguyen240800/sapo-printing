//! Integration Tests Module Entry Point
//!
//! This module structure allows Cargo to discover and run integration tests.

mod secrets {
    #[cfg(target_os = "windows")]
    mod windows_integration_test;

    #[cfg(target_os = "macos")]
    mod macos_integration_test;

    #[cfg(target_os = "linux")]
    mod linux_integration_test;
}

mod downloader_integration_test;
mod renderer_integration_test;
