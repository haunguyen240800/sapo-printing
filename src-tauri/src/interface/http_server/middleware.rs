//! Auth middleware — verify Bearer token.

use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::Response,
};

use super::handlers::{ApiError, ApiErrorResponse};
use super::state::HttpServerState;

/// Extract Bearer token → verify via ApiTokenManager. Inject origin vào extensions.
pub async fn require_auth(
    State(state): State<HttpServerState>,
    mut req: Request,
    next: Next,
) -> Result<Response, ApiErrorResponse> {
    let token = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    let Some(token) = token else {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "missing_token",
            "missing Bearer token",
        ));
    };

    match state.token_manager.verify_token(&token) {
        Some(origin) => {
            req.extensions_mut().insert(AuthenticatedOrigin(origin));
            Ok(next.run(req).await)
        }
        None => Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_token",
            "invalid or expired token",
        )),
    }
}

#[derive(Debug, Clone)]
pub struct AuthenticatedOrigin(pub String);
