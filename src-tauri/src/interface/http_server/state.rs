use std::sync::Arc;

use crate::application::ports::ApiTokenPort;
use crate::application::use_cases::{
    CancelPrintJobUseCase, CreatePrintJobUseCase, GetJobStatusUseCase,
};

use super::sse::SseBroadcaster;

#[derive(Clone)]
pub struct HttpServerState {
    pub token_manager: Arc<dyn ApiTokenPort>,
    pub app_version: &'static str,
    pub agent_port: u16,
    pub sse_broadcaster: Option<Arc<SseBroadcaster>>,
    pub create_print_job_uc: Arc<CreatePrintJobUseCase>,
    pub get_job_status_uc: Arc<GetJobStatusUseCase>,
    pub cancel_print_job_uc: Arc<CancelPrintJobUseCase>,
}
