//! Auth middleware — verify Bearer token.

use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::Response,
};

use super::state::HttpServerState;

/// Extract Bearer token → verify via ApiTokenManager. Inject origin vào extensions.
pub async fn require_auth(
    State(state): State<HttpServerState>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let token = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    let Some(token) = token else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    match state.token_manager.verify_token(&token) {
        Some(origin) => {
            req.extensions_mut().insert(AuthenticatedOrigin(origin));
            Ok(next.run(req).await)
        }
        None => Err(StatusCode::UNAUTHORIZED),
    }
}

#[derive(Debug, Clone)]
pub struct AuthenticatedOrigin(pub String);
