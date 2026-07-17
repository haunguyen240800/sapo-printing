//! REST handlers.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::application::services::PairError;

use super::cors;
use super::state::HttpServerState;

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
) -> Result<Json<PairResponseBody>, (StatusCode, String)> {
    // Origin header trong request phải khớp body.origin.
    let origin_hdr = headers
        .get("origin")
        .and_then(|v| v.to_str().ok())
        .ok_or((StatusCode::BAD_REQUEST, "missing Origin header".into()))?;
    if origin_hdr != body.origin {
        return Err((
            StatusCode::BAD_REQUEST,
            "Origin header mismatch with body".into(),
        ));
    }
    if !cors::check(&body.origin) {
        return Err((StatusCode::FORBIDDEN, "origin not allowed".into()));
    }

    match state.token_manager.request_pair(&body.origin).await {
        Ok(resp) => Ok(Json(PairResponseBody {
            api_token: resp.api_token,
            expires_at: resp.expires_at,
        })),
        Err(PairError::RateLimited) => {
            Err((StatusCode::TOO_MANY_REQUESTS, "rate limited".into()))
        }
        Err(PairError::UserDenied) => Err((StatusCode::FORBIDDEN, "user denied".into())),
        Err(PairError::Timeout) => Err((StatusCode::REQUEST_TIMEOUT, "user did not respond".into())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

// ==================== /api/v1/jobs (stub) ====================
// Sprint 6 sẽ wire vào CreatePrintJobUseCase.

#[derive(Deserialize)]
pub struct CreateJobsRequest {
    pub printer_name: String,
    pub document_urls: Vec<String>,
}

#[derive(Serialize)]
pub struct CreateJobsResponse {
    pub job_ids: Vec<String>,
}

pub async fn create_jobs(
    State(_state): State<HttpServerState>,
    Json(_body): Json<CreateJobsRequest>,
) -> Result<Json<CreateJobsResponse>, (StatusCode, String)> {
    Err((
        StatusCode::NOT_IMPLEMENTED,
        "job creation wired in Sprint 6".into(),
    ))
}

pub async fn get_job(
    State(_state): State<HttpServerState>,
    Path(_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    Err((
        StatusCode::NOT_IMPLEMENTED,
        "job status wired in Sprint 6".into(),
    ))
}

pub async fn list_printers(
    State(_state): State<HttpServerState>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    Err((
        StatusCode::NOT_IMPLEMENTED,
        "printers wired in Sprint 6".into(),
    ))
}
