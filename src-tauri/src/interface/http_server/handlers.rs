//! REST handlers.

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use crate::application::models::PrintJobCreateRequest;
use crate::application::errors::Error;
use crate::application::ports::PairError;

use super::cors;
use super::state::HttpServerState;

// ==================== Error envelope ====================

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
}

impl ApiError {
    pub fn new(
        status: StatusCode,
        code: &'static str,
        message: impl Into<String>,
    ) -> ApiErrorResponse {
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
}

pub async fn ping(State(state): State<HttpServerState>) -> impl IntoResponse {
    Json(PingResponse {
        status: "ok",
        version: state.app_version,
    })
}

// ==================== /api/v1/pair ====================

#[derive(Serialize)]
pub struct PairResponseBody {
    pub api_token: String,
    pub expires_at: i64,
}

pub async fn pair(
    State(state): State<HttpServerState>,
    headers: HeaderMap,
) -> Result<Json<PairResponseBody>, ApiErrorResponse> {
    let origin = headers
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "missing_origin",
                "missing Origin header",
            )
        })?;
    if !cors::check(origin) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "origin_not_allowed",
            "origin not allowed",
        ));
    }

    match state.token_manager.request_pair(origin).await {
        Ok(resp) => Ok(Json(PairResponseBody {
            api_token: resp.api_token,
            expires_at: resp.expires_at,
        })),
        Err(PairError::UserDenied) => Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "user_denied",
            "user denied",
        )),
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
        Err(e) => Err(ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            e.to_string(),
        )),
    }
}

// ==================== /api/v1/jobs ====================

#[derive(Deserialize)]
pub struct CreateJobRequest {
    pub document_url: String,
}

#[derive(Serialize)]
pub struct CreateJobResponse {
    pub job_id: String,
}

pub async fn create_job(
    State(state): State<HttpServerState>,
    Json(body): Json<CreateJobRequest>,
) -> Result<Json<CreateJobResponse>, ApiErrorResponse> {
    let use_case = state.create_print_job_uc.clone();
    let request = PrintJobCreateRequest {
        pdf_url: body.document_url,
    };

    let result = tokio::task::spawn_blocking(move || use_case.execute(request))
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "task_join",
                e.to_string(),
            )
        })?;

    result
        .map(|id| {
            Json(CreateJobResponse {
                job_id: id.to_string(),
            })
        })
        .map_err(map_app_error)
}

pub async fn get_job(
    State(state): State<HttpServerState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiErrorResponse> {
    let use_case = state.get_job_status_uc.clone();
    let result = tokio::task::spawn_blocking(move || use_case.execute(&id))
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "task_join",
                e.to_string(),
            )
        })?;

    let dto = result.map_err(map_app_error)?;
    serde_json::to_value(dto).map(Json).map_err(|e| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "serialize_error",
            e.to_string(),
        )
    })
}

fn map_app_error(e: Error) -> ApiErrorResponse {
    let (status, code) = match &e {
        Error::EmptyJobList => (StatusCode::BAD_REQUEST, "empty_job_list"),
        Error::TooManyJobs { .. } => (StatusCode::BAD_REQUEST, "too_many_jobs"),
        Error::InvalidJobId { .. } => (StatusCode::BAD_REQUEST, "invalid_job_id"),
        Error::ValidationError { .. } => (StatusCode::BAD_REQUEST, "validation_error"),
        Error::JobNotFound { .. } => (StatusCode::NOT_FOUND, "not_found"),
        Error::PrinterNotAvailable { .. } => (StatusCode::BAD_REQUEST, "printer_not_available"),
        Error::CannotCancelCompleted { .. } => (StatusCode::CONFLICT, "cannot_cancel_completed"),
        Error::CannotCancelFailed { .. } => (StatusCode::CONFLICT, "cannot_cancel_failed"),
        Error::CannotCancelCancelled { .. } => (StatusCode::CONFLICT, "cannot_cancel_cancelled"),
        Error::DomainRuleViolation { .. } => {
            (StatusCode::UNPROCESSABLE_ENTITY, "domain_rule_violation")
        }
        Error::RepositoryError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "repository_error"),
        Error::EventStoreError { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "event_store_error"),
        Error::EventBusError { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "event_bus_error"),
        Error::MetricsError { .. } => (StatusCode::INTERNAL_SERVER_ERROR, "metrics_error"),
        Error::PrintJobError(_) => (StatusCode::INTERNAL_SERVER_ERROR, "print_job_error"),
        // Port / infrastructure errors (e.g. download timeout, invalid input from infra)
        Error::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
        Error::InvalidInput(_) => (StatusCode::BAD_REQUEST, "invalid_input"),
        Error::Unavailable(_) => (StatusCode::SERVICE_UNAVAILABLE, "service_unavailable"),
        Error::Timeout(_) => (StatusCode::GATEWAY_TIMEOUT, "timeout"),
        Error::Operation(_) => (StatusCode::INTERNAL_SERVER_ERROR, "operation_error"),
    };
    ApiError::new(status, code, e.to_string())
}
