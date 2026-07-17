//! Shared application state cho Axum handlers.

use std::sync::Arc;

use crate::application::services::ApiTokenManager;

use super::sse::SseBroadcaster;

#[derive(Clone)]
pub struct HttpServerState {
    pub token_manager: Arc<ApiTokenManager>,
    pub app_version: &'static str,
    pub min_webapp_version: &'static str,
    pub agent_port: u16,
    pub sse_broadcaster: Option<Arc<SseBroadcaster>>,
    // Sprint 6: job_repo, printer_manager, event_bus
}
