use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::application::ports::ApiTokenPort;
use crate::application::ports::event_bus::EventBus;
use crate::application::use_cases::{
    CancelPrintJobUseCase, CreatePrintJobUseCase, GetJobStatusUseCase,
};
use crate::infrastructure::errors::InfrastructureError;

use super::server::{self, AgentMetadata};
use super::sse::SseBroadcaster;
use super::state::HttpServerState;

pub const JOB_STATUS_EVENTS: &[&str] = &["PrintJobCompleted", "PrintJobFailed"];

pub struct BootstrapResult {
    pub token_manager: Arc<dyn ApiTokenPort>,
    pub sse_broadcaster: Arc<SseBroadcaster>,
    pub port: u16,
}

pub async fn start(
    data_dir: &Path,
    token_manager: Arc<dyn ApiTokenPort>,
    event_bus: Arc<dyn EventBus>,
    create_print_job_uc: Arc<CreatePrintJobUseCase>,
    get_job_status_uc: Arc<GetJobStatusUseCase>,
    cancel_print_job_uc: Arc<CancelPrintJobUseCase>,
    app_version: &'static str,
) -> Result<BootstrapResult, InfrastructureError> {
    let broadcaster = SseBroadcaster::new();
    for event_type in JOB_STATUS_EVENTS {
        event_bus.subscribe(
            event_type,
            broadcaster.clone() as Arc<dyn crate::application::ports::event_bus::EventHandler>,
        );
    }

    let broadcaster_for_state = broadcaster.clone();
    let tm_for_state = token_manager.clone();
    let handles = server::start_server(move |port| HttpServerState {
        token_manager: tm_for_state,
        app_version,
        agent_port: port,
        sse_broadcaster: Some(broadcaster_for_state),
        create_print_job_uc,
        get_job_status_uc,
        cancel_print_job_uc,
    })
    .await?;

    AgentMetadata {
        port: handles.port,
        version: app_version.into(),
        started_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0),
    }
    .write(data_dir)?;

    Ok(BootstrapResult {
        token_manager,
        sse_broadcaster: broadcaster,
        port: handles.port,
    })
}
