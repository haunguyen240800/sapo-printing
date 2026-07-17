//! Axum router — endpoints + CORS + auth middleware.

use axum::{
    middleware,
    routing::{get, post},
    Router,
};

use super::cors;
use super::handlers;
use super::middleware as auth_mw;
use super::sse;
use super::state::HttpServerState;

pub fn build(state: HttpServerState) -> Router {
    let public = Router::new()
        .route("/api/v1/ping", get(handlers::ping))
        .route("/api/v1/pair", post(handlers::pair))
        // SSE: auth qua query param, không dùng require_auth middleware.
        .route("/api/v1/events", get(sse::sse_stream));

    let protected = Router::new()
        .route("/api/v1/jobs", post(handlers::create_jobs))
        .route("/api/v1/jobs/:id", get(handlers::get_job))
        .route("/api/v1/printers", get(handlers::list_printers))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_mw::require_auth,
        ));

    public
        .merge(protected)
        .with_state(state)
        .layer(cors::build_layer())
}
