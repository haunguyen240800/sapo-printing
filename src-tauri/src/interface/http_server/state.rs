//! Shared application state cho Axum handlers.
//!
//! Interface layer chỉ giữ Application-layer refs (UseCaseFactory + services)
//! — không import Domain repos trực tiếp.

use std::sync::Arc;

use crate::application::services::{ApiTokenManager, UseCaseFactory};

use super::sse::SseBroadcaster;

#[derive(Clone)]
pub struct HttpServerState {
    pub token_manager: Arc<ApiTokenManager>,
    pub app_version: &'static str,
    pub min_webapp_version: &'static str,
    pub agent_port: u16,
    pub sse_broadcaster: Option<Arc<SseBroadcaster>>,
    pub use_cases: Arc<UseCaseFactory>,
}
