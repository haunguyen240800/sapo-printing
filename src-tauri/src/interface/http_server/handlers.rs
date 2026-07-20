//! REST handlers.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::application::dto::create_print_job_request::CreatePrintJobRequest;
use crate::application::errors::ApplicationError;
use crate::application::services::PairError;

use super::cors;
use super::state::HttpServerState;

// ==================== Error envelope ====================

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> ApiErrorResponse {
        ApiErrorResponse {
            status,
            body: ApiError {
                code,
                message: message.into(),
            },
        }
    }
}

pub struct ApiErrorResponse {
    status: StatusCode,
    body: ApiError,
}

impl IntoResponse for ApiErrorResponse {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

// ==================== /api/v1/ping ====================

#[derive(Serialize)]
pub struct PingResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub min_webapp_version: &'static str,
    pub features: Vec<&'static str>,
    pub port: u16,
}

pub async fn ping(State(state): State<HttpServerState>) -> impl IntoResponse {
    Json(PingResponse {
        status: "ok",
        version: state.app_version,
        min_webapp_version: state.min_webapp_version,
        features: vec!["sse", "pair"],
        port: state.agent_port,
    })
}

// ==================== /api/v1/pair ====================

#[derive(Deserialize)]
pub struct PairRequest {
    pub origin: String,
}

#[derive(Serialize)]
pub struct PairResponseBody {
    pub api_token: String,
    pub expires_at: i64,
}

pub async fn pair(
    State(state): State<HttpServerState>,
    headers: HeaderMap,
    Json(body): Json<PairRequest>,
) -> Result<Json<PairResponseBody>, ApiErrorResponse> {
    let origin_hdr = headers
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "missing_origin", "missing Origin header"))?;
    if origin_hdr != body.origin {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "origin_mismatch",
            "Origin header mismatch with body",
        ));
    }
    if !cors::check(&body.origin) {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "origin_not_allowed", "origin not allowed"));
    }

    match state.token_manager.request_pair(&body.origin).await {
        Ok(resp) => Ok(Json(PairResponseBody {
            api_token: resp.api_token,
            expires_at: resp.expires_at,
        })),
        Err(PairError::UserDenied) => Err(ApiError::new(StatusCode::FORBIDDEN, "user_denied", "user denied")),
        Err(PairError::Timeout) => Err(ApiError::new(
            StatusCode::REQUEST_TIMEOUT,
            "pair_timeout",
            "user did not respond",
        )),
        Err(PairError::NoUiSubscriber) => Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "no_ui_subscriber",
            "no ui subscriber to receive pair request",
        )),
        Err(e) => Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", e.to_string())),
    }
}

// ==================== /api/v1/jobs ====================

#[derive(Deserialize)]
pub struct CreateJobsRequest {
    pub printer_name: String,
    pub document_urls: Vec<String>,
    #[serde(default)]
    pub output_path: Option<String>,
}

#[derive(Serialize)]
pub struct CreateJobsResponse {
    pub job_ids: Vec<String>,
}

pub async fn create_jobs(
    State(state): State<HttpServerState>,
    Json(body): Json<CreateJobsRequest>,
) -> Result<Json<CreateJobsResponse>, ApiErrorResponse> {
    let use_case = state.use_cases.create_print_job();
    let request = CreatePrintJobRequest {
        pdf_urls: body.document_urls,
        printer_name: body.printer_name,
        output_path: body.output_path,
    };

    let result = tokio::task::spawn_blocking(move || use_case.execute(request))
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "task_join", e.to_string()))?;

    result
        .map(|ids| {
            Json(CreateJobsResponse {
                job_ids: ids.into_iter().map(|id| id.to_string()).collect(),
            })
        })
        .map_err(map_app_error)
}

pub async fn get_job(
    State(state): State<HttpServerState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiErrorResponse> {
    let use_case = state.use_cases.get_job_status();
    let result = tokio::task::spawn_blocking(move || use_case.execute(&id))
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "task_join", e.to_string()))?;

    let dto = result.map_err(map_app_error)?;
    serde_json::to_value(dto)
        .map(Json)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "serialize_error", e.to_string()))
}

pub async fn list_printers(
    State(state): State<HttpServerState>,
) -> Result<Json<serde_json::Value>, ApiErrorResponse> {
    let use_case = state.use_cases.list_printers();
    let result = tokio::task::spawn_blocking(move || use_case.execute())
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "task_join", e.to_string()))?;

    let list = result.map_err(map_app_error)?;
    serde_json::to_value(list)
        .map(Json)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "serialize_error", e.to_string()))
}

fn map_app_error(e: ApplicationError) -> ApiErrorResponse {
    let (status, code) = match &e {
        ApplicationError::EmptyJobList => (StatusCode::BAD_REQUEST, "empty_job_list"),
        ApplicationError::TooManyJobs { .. } => (StatusCode::BAD_REQUEST, "too_many_jobs"),
        ApplicationError::InvalidJobId { .. } => (StatusCode::BAD_REQUEST, "invalid_job_id"),
        ApplicationError::ValidationError { .. } => (StatusCode::BAD_REQUEST, "validation_error"),
        ApplicationError::JobNotFound { .. } => (StatusCode::NOT_FOUND, "not_found"),
        ApplicationError::PrinterNotAvailable { .. } => {
            (StatusCode::BAD_REQUEST, "printer_not_available")
        }
        ApplicationError::CannotCancelCompleted { .. } => (StatusCode::CONFLICT, "cannot_cancel_completed"),
        ApplicationError::CannotCancelFailed { .. } => (StatusCode::CONFLICT, "cannot_cancel_failed"),
        ApplicationError::CannotCancelCancelled { .. } => (StatusCode::CONFLICT, "cannot_cancel_cancelled"),
        ApplicationError::DomainRuleViolation { .. } => (StatusCode::UNPROCESSABLE_ENTITY, "domain_rule_violation"),
        ApplicationError::RepositoryError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "repository_error"),
        ApplicationError::EventStoreError { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "event_store_error"),
        ApplicationError::EventBusError { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "event_bus_error"),
        ApplicationError::MetricsError { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "metrics_error"),
        ApplicationError::PrintJobError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "print_job_error"),
    };
    ApiError::new(status, code, e.to_string())
}
